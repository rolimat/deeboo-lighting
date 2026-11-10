//! The engine thread: one blocking loop over a bounded channel. It wakes for an input or for the
//! engine's next deadline, never on a timer of its own (spec §6.1, §13).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use light_core::clock::PhraseBars;
use light_core::detect::{BandAnalyzer, BeatFeatures, DetectorParams, Moment, MomentDetector};
use light_core::engine::{Engine, EngineConfig, EngineStatus, UiCommand};
use light_core::library::Library;
use light_core::model::{Palette, Sequence};
use light_core::paint::PaintCommand;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::audio::{AudioError, AudioIn, RING_SECONDS};
use crate::clock::MonoClock;
use crate::commands::{AppState, AudioStatus, ClipDto, MidiStatus, Selection, Status, StripStatus};
use crate::midi::{MidiEvent, MidiIn, QUEUE};
use crate::settings::{Settings, detector_params};
use crate::strip::{self, StripMsg};
use crate::strip_store::StripConfig;
use crate::tray::{TrayInput, TrayView, tray_deadline, tray_view};

/// Everything that can wake the engine thread.
pub enum Input {
    Midi(MidiEvent),
    /// The OS's MIDI sources changed (our own virtual port excluded).
    SourcesChanged(Vec<String>),
    Ui(UiCommand),
    Settings(Settings),
    /// The editor saved the library (already validated and persisted).
    Library(Box<Library>),
    PopoverShown,
    /// The editor's Detection section opened (`true`) or closed / the window went away (`false`).
    DetectionWatch(bool),
    /// The editor asks to flash one segment (1-based) so the user can tell which piece it is.
    Identify(u8),
    /// "Show on strip" in the editor's Palettes tab: this palette and sequence replace the show
    /// (`Some`, already validated) until `None`.
    PreviewPalette(Option<Box<(Palette, Sequence)>>),
    /// Setup's latency calibration: the strip flashes white on every beat (`true`) or goes back to the show.
    Calibrate(bool),
    StripState(StripStatus),
    /// The wizard saved a new strip file: reconnect with it.
    StripChanged(StripConfig),
    /// The wizard needs the strip's single LAN connection (`true`) or lets it go (`false`).
    StripPause(bool),
    Quit,
}

/// What the engine needs from the MIDI input; a fake in tests.
pub trait MidiPort {
    fn connected_source(&self) -> Option<String>;
    /// Whether the OS lists a MIDI source called `name` right now (the connection may be stale).
    fn source_available(&self, name: &str) -> bool;
    fn reconnect(&mut self, source: Option<&str>);
    /// Messages lost because the engine fell behind.
    fn dropped(&self) -> u64;
}

/// What an opened capture reports.
pub struct AudioInfo {
    pub rate: u32,
    pub name: String,
}

/// What the engine needs from audio capture; a fake in tests.
pub trait AudioPort {
    fn open(&mut self, device: Option<&str>) -> Result<AudioInfo, AudioError>;
    fn close(&mut self);
    /// Moves what the callback has pushed into `buf`, never past its capacity.
    fn drain(&mut self, buf: &mut Vec<f32>);
    /// Samples lost because the engine fell behind.
    fn overflow(&self) -> u64;
}

/// The microphone is open only while there is something to follow.
fn capture_wanted(playing: bool, auto: bool, lights_on: bool) -> bool {
    playing && auto && lights_on
}

#[derive(Clone, PartialEq)]
enum Capture {
    Closed,
    Open(String),
    /// Tried while wanted and failed; retried only after it was unwanted or the input changed.
    Failed,
}

/// Band analyser and moment detector for one sample rate.
struct Detect {
    rate: u32,
    bands: BandAnalyzer,
    moment: MomentDetector,
}

impl Detect {
    #[allow(clippy::cast_precision_loss)] // rates are far below 2^24
    fn new(rate: u32, params: DetectorParams) -> Self {
        let bands = BandAnalyzer::new(rate);
        let hop_s = bands.hop_len() as f32 / rate as f32;
        Self {
            rate,
            bands,
            moment: MomentDetector::new(params, hop_s),
        }
    }
}

/// Runs the beat's audio through the detector: its moment, or `None` without audio.
fn detect_beat<A: AudioPort>(
    audio: &mut A,
    detect: &mut Option<Detect>,
    buf: &mut Vec<f32>,
    level_db: &mut Option<f32>,
    features: &mut Option<(BeatFeatures, Moment)>,
) -> Option<Moment> {
    let d = detect.as_mut()?;
    buf.clear();
    audio.drain(buf);
    if buf.is_empty() {
        return None;
    }
    let peak = buf.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    *level_db = Some((20.0 * peak.max(1e-6).log10()).round());
    d.bands.process(buf, &mut |f| d.moment.push(f));
    let (moment, f) = d.moment.on_beat();
    *features = f.map(|f| (f, moment));
    Some(moment)
}

/// What the Detection panel draws for one beat; sent only while it is open.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectorEvent {
    pub kick_db: f32,
    pub sub_db: f32,
    pub high_db: f32,
    pub kick_ref_db: f32,
    pub sub_ref_db: f32,
    pub high_ref_db: f32,
    pub moment: Moment,
}

type MeterSink = Box<dyn FnMut(&DetectorEvent)>;

/// The editor gets at most 15 detector events per second.
const MIN_METER_GAP_US: u64 = 66_667;
const DETECTOR_EVENT: &str = "detector";
const MIDI_SOURCES_EVENT: &str = "midi-sources";

/// How long an identify flash stays before the scene is repainted.
const IDENTIFY_FLASH_US: u64 = 1_200_000;
/// A new identify inside this window is dropped: the strip loses back-to-back messages.
const IDENTIFY_GATE_US: u64 = 300_000;

/// The popover gets at most 30 updates per second.
const MIN_PUBLISH_GAP_US: u64 = 33_333;
/// Segments of the factory strip, used until a strip file says otherwise.
pub const DEFAULT_SEGMENTS: u8 = 10;
const STATUS_EVENT: &str = "status";

/// Decides when a status update goes to the popover: only while it is visible, coalesced.
#[derive(Default)]
struct Publisher {
    visible: bool,
    dirty: bool,
    last_us: Option<u64>,
}

impl Publisher {
    fn touch(&mut self) {
        self.dirty = true;
    }

    fn set_visible(&mut self, visible: bool) {
        if visible && !self.visible {
            self.dirty = true;
        }
        self.visible = visible;
    }

    /// True once per publish: the caller must send the status now.
    fn due(&mut self, now_us: u64) -> bool {
        if !self.visible || !self.dirty || self.deadline().is_some_and(|d| d > now_us) {
            return false;
        }
        self.dirty = false;
        self.last_us = Some(now_us);
        true
    }

    /// When the pending update may go out.
    fn deadline(&self) -> Option<u64> {
        (self.visible && self.dirty).then(|| self.last_us.map_or(0, |t| t + MIN_PUBLISH_GAP_US))
    }
}

/// How long to block for: `None` means wait for an input with no timeout.
fn timeout(deadline_us: Option<u64>, now_us: u64) -> Option<Duration> {
    deadline_us.map(|d| Duration::from_micros(d.saturating_sub(now_us)))
}

/// A configured source that is not connected (it appeared after launch) needs another try.
fn needs_reconnect(wanted: Option<&str>, connected: Option<&str>) -> bool {
    wanted.is_some() && wanted != connected
}

fn phrase(bars: u8) -> PhraseBars {
    match bars {
        4 => PhraseBars::Four,
        16 => PhraseBars::Sixteen,
        _ => PhraseBars::Eight,
    }
}

/// A fresh engine for `segments` with the settings' show parameters.
fn engine_for(settings: &Settings, segments: u8, lights_on: bool) -> Engine {
    // Launch time (µs) seeds random order and generated sequences: shows differ between sessions.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(0));
    Engine::with_seed(
        EngineConfig {
            segments,
            phrase: phrase(settings.phrase_bars),
            lead_us: u64::from(settings.lead_ms) * 1000,
            brightness_pct: settings.brightness_pct,
            lights_on,
        },
        seed,
    )
}

/// The engine status as the popover sees it: BPM to a tenth, so jitter does not repaint.
fn snapshot(engine: &Engine) -> EngineStatus {
    let mut s = engine.status();
    s.bpm = s.bpm.map(|b| (b * 10.0).round() / 10.0);
    s
}

/// The thread's state; generic over the MIDI input and the status sink so tests can fake both.
pub struct Core<M: MidiPort, A: AudioPort, E: FnMut(&Status)> {
    engine: Engine,
    /// Segment count the engine plans for.
    segments: u8,
    midi: M,
    audio: A,
    capture: Capture,
    detect: Option<Detect>,
    buf: Vec<f32>,
    level_db: Option<f32>,
    settings: Settings,
    strip: StripStatus,
    strip_tx: Option<UnboundedSender<StripMsg>>,
    publisher: Publisher,
    emit: E,
    last: EngineStatus,
    tray: Option<TrayLink>,
    tray_due: Option<u64>,
    /// The OS listed the configured source at the last check (popover show or settings change).
    source_present: bool,
    /// The wizard holds the strip: the engine keeps running but sends nothing.
    strip_paused: bool,
    /// The saved library: kept to rebuild the engine's look after a segment-count change.
    library: Option<Library>,
    /// When the last identify flash started; the scene is repainted `IDENTIFY_FLASH_US` later.
    identify_at: Option<u64>,
    /// The editor's Detection section is open: only then are `detector` events produced.
    watching: bool,
    meter_sink: Option<MeterSink>,
    meter_last_us: Option<u64>,
    /// Told the strip state on every change, popover visible or not (the editor reads it).
    strip_sink: Option<StripSink>,
    strip_told: Option<StripStatus>,
    sources_sink: Option<SourcesSink>,
    choice_sink: Option<ChoiceSink>,
}

/// Told every new list of MIDI sources (the UI listens).
type SourcesSink = Box<dyn FnMut(&[String])>;
/// Told the source the engine chose by itself (to persist it).
type ChoiceSink = Box<dyn FnMut(&str)>;

type StripSink = Box<dyn FnMut(&StripStatus)>;

/// Paints the menu bar: called with the previous and the new view, only when they differ.
type TraySink = Box<dyn FnMut(Option<&TrayView>, &TrayView)>;

struct TrayLink {
    sink: TraySink,
    shown: Option<TrayView>,
}

impl<M: MidiPort, A: AudioPort, E: FnMut(&Status)> Core<M, A, E> {
    pub fn new(
        settings: Settings,
        segments: u8,
        strip_tx: Option<UnboundedSender<StripMsg>>,
        midi: M,
        audio: A,
        emit: E,
        now_us: u64,
    ) -> Self {
        let engine = engine_for(&settings, segments, settings.lights_on);
        let strip = if strip_tx.is_some() {
            StripStatus::Connecting
        } else {
            StripStatus::NotConfigured
        };
        let last = snapshot(&engine);
        let source_present = settings
            .midi_source
            .as_deref()
            .is_none_or(|n| midi.source_available(n));
        let mut core = Self {
            engine,
            segments,
            midi,
            audio,
            capture: Capture::Closed,
            detect: None,
            buf: Vec::new(),
            level_db: None,
            settings,
            strip,
            strip_tx,
            publisher: Publisher::default(),
            emit,
            last,
            tray: None,
            tray_due: None,
            source_present,
            strip_paused: false,
            library: None,
            identify_at: None,
            watching: false,
            meter_sink: None,
            meter_last_us: None,
            strip_sink: None,
            strip_told: None,
            sources_sink: None,
            choice_sink: None,
        };
        core.replan(now_us);
        core
    }

    pub fn set_midi_sinks(&mut self, sources: SourcesSink, choice: ChoiceSink) {
        self.sources_sink = Some(sources);
        self.choice_sink = Some(choice);
    }

    /// The list is the truth: connect a source that showed up (or came back), pick the only one
    /// when none is chosen. Repeated notifications change nothing.
    fn on_sources(&mut self, list: &[String]) {
        if let Some(sink) = &mut self.sources_sink {
            sink(list);
        }
        match self.settings.midi_source.clone() {
            None => {
                if let [only] = list {
                    self.settings.midi_source = Some(only.clone());
                    if let Some(sink) = &mut self.choice_sink {
                        sink(only);
                    }
                    self.midi.reconnect(Some(only));
                    self.source_present = true;
                }
            }
            Some(wanted) => {
                let present = list.contains(&wanted);
                let stale = !self.source_present
                    || needs_reconnect(Some(&wanted), self.midi.connected_source().as_deref());
                if present && stale {
                    self.midi.reconnect(Some(&wanted));
                }
                self.source_present = present;
            }
        }
        self.publisher.touch();
    }

    /// The scene for the current state (rest, or all off); the live clock replans on its next beat.
    fn replan(&mut self, now_us: u64) {
        let on = self.engine.status().lights_on;
        let cmds = self.engine.command(now_us, UiCommand::Lights(on));
        self.flush(cmds);
    }

    fn flush(&self, cmds: Vec<PaintCommand>) {
        let Some(tx) = &self.strip_tx else { return };
        if !matches!(self.strip, StripStatus::Connected { .. }) {
            return;
        }
        for cmd in cmds {
            let _ = tx.send(StripMsg::Paint(cmd));
        }
    }

    pub fn status(&self) -> Status {
        let s = &self.last;
        Status {
            strip: self.strip.clone(),
            bpm: s.bpm,
            playing: s.playing,
            beat_in_bar: s.pos.map(|p| p.beat_in_bar),
            bar_in_phrase: s.pos.map(|p| p.bar_in_phrase),
            phrase_bars: s.phrase.bars(),
            moment: s.moment.into(),
            auto: s.auto,
            predicted: s.predicted,
            lights_on: s.lights_on,
            midi: MidiStatus {
                source: self.settings.midi_source.clone(),
                ok: self.midi_ok(),
            },
            audio: match &self.capture {
                Capture::Open(name) => AudioStatus {
                    ok: true,
                    device: Some(name.clone()),
                    level_db: self.level_db,
                },
                _ => AudioStatus::default(),
            },
            selection: self.library.as_ref().map_or_else(
                || Selection::of(&Library::builtin(self.segments)),
                |lib| Selection::in_force(lib, s.palette),
            ),
            clip: s.clip.map(|c| ClipDto {
                moment: c.moment.into(),
                index: c.index,
                elapsed: c.elapsed,
                total: c.total,
            }),
        }
    }

    /// Opens or closes the microphone to match the state; runs after every input.
    fn sync_capture(&mut self) {
        let s = &self.last;
        if capture_wanted(s.playing, s.auto, s.lights_on) {
            if self.capture != Capture::Closed {
                return;
            }
            match self.audio.open(self.settings.audio_input.as_deref()) {
                Ok(info) => {
                    if self.detect.as_ref().is_none_or(|d| d.rate != info.rate) {
                        self.detect = Some(Detect::new(info.rate, detector_params(&self.settings)));
                    }
                    self.buf = Vec::with_capacity((info.rate * RING_SECONDS) as usize);
                    self.capture = Capture::Open(info.name);
                }
                Err(e) => {
                    log::warn!("audio not available: {e}");
                    self.capture = Capture::Failed;
                }
            }
            self.publisher.touch();
        } else if self.capture != Capture::Closed {
            if matches!(self.capture, Capture::Open(_)) {
                log::debug!("audio samples lost so far: {}", self.audio.overflow());
                self.audio.close();
            }
            self.capture = Capture::Closed;
            self.level_db = None;
            self.publisher.touch();
        }
    }

    /// Starts driving the menu bar through `sink`.
    pub fn set_tray(&mut self, sink: TraySink, now_us: u64) {
        self.tray = Some(TrayLink { sink, shown: None });
        self.update_tray(now_us);
    }

    pub fn set_strip_sink(&mut self, sink: StripSink) {
        self.strip_sink = Some(sink);
        self.tell_strip();
    }

    fn tell_strip(&mut self) {
        if let Some(sink) = &mut self.strip_sink
            && self.strip_told.as_ref() != Some(&self.strip)
        {
            sink(&self.strip);
            self.strip_told = Some(self.strip.clone());
        }
    }

    pub fn set_meters(&mut self, sink: MeterSink) {
        self.meter_sink = Some(sink);
    }

    /// Sends this beat's features to the editor if it is watching and the rate cap allows.
    fn meter(&mut self, f: &BeatFeatures, moment: Moment, now_us: u64) {
        if !self.watching
            || self
                .meter_last_us
                .is_some_and(|t| now_us < t + MIN_METER_GAP_US)
        {
            return;
        }
        let Some(sink) = &mut self.meter_sink else {
            return;
        };
        self.meter_last_us = Some(now_us);
        sink(&DetectorEvent {
            kick_db: f.kick_db,
            sub_db: f.sub_db,
            high_db: f.high_db,
            kick_ref_db: f.kick_ref_db,
            sub_ref_db: f.sub_ref_db,
            high_ref_db: f.high_ref_db,
            moment,
        });
    }

    fn midi_ok(&self) -> bool {
        self.settings.midi_source.is_none()
            || (self.source_present && self.settings.midi_source == self.midi.connected_source())
    }

    fn update_tray(&mut self, now_us: u64) {
        let midi_ok = self.midi_ok();
        let s = &self.last;
        let Some(link) = &mut self.tray else { return };
        let input = TrayInput {
            midi_ok,
            strip_ok: matches!(self.strip, StripStatus::Connected { .. }),
            playing: s.playing,
            bpm: s.bpm,
            last_beat_us: s.last_beat_us,
            moment: s.moment,
            forced: !s.auto,
        };
        let view = tray_view(&input, now_us);
        if link.shown.as_ref() != Some(&view) {
            (link.sink)(link.shown.as_ref(), &view);
            link.shown = Some(view);
        }
        self.tray_due = tray_deadline(&input, now_us);
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.publisher.set_visible(visible);
    }

    /// False once the thread should stop.
    pub fn handle(&mut self, input: Input, now_us: u64) -> bool {
        match input {
            Input::Midi(e) => {
                let mut features = None;
                let cmds = self.engine.on_midi(e.at_us, e.status, || {
                    detect_beat(
                        &mut self.audio,
                        &mut self.detect,
                        &mut self.buf,
                        &mut self.level_db,
                        &mut features,
                    )
                });
                self.flush(cmds);
                if let Some((f, moment)) = features {
                    self.meter(&f, moment, now_us);
                }
            }
            Input::SourcesChanged(list) => self.on_sources(&list),
            Input::Ui(cmd) => {
                // A rejected key is not retried: only pairing again helps.
                if let (
                    UiCommand::Lights(true),
                    StripStatus::Error | StripStatus::NotFound,
                    Some(tx),
                ) = (cmd, &self.strip, &self.strip_tx)
                {
                    let _ = tx.send(StripMsg::Connect);
                }
                let cmds = self.engine.command(now_us, cmd);
                self.flush(cmds);
                self.publisher.touch();
            }
            Input::Settings(new) => self.apply_settings(new, now_us),
            Input::DetectionWatch(on) => self.watching = on,
            Input::PopoverShown => {
                log::debug!("MIDI messages dropped so far: {}", self.midi.dropped());
                self.check_midi_source();
                self.publisher.touch();
            }
            Input::Identify(segment) => self.identify(segment, now_us),
            Input::PreviewPalette(p) => {
                let cmds = self.engine.set_preview(now_us, p.map(|b| *b));
                self.flush(cmds);
            }
            Input::Calibrate(on) => {
                let cmds = self.engine.calibrate(now_us, on);
                self.flush(cmds);
            }
            Input::StripState(_) if self.strip_paused => {
                // A report from before the pause: the task will say so again after the resume.
            }
            Input::StripState(s) => {
                let connected = matches!(s, StripStatus::Connected { .. });
                self.strip = s;
                if connected {
                    self.engine.strip_reconnected();
                    self.replan(now_us);
                }
                self.publisher.touch();
            }
            Input::Library(lib) => {
                let cmds = self.engine.set_library(now_us, &lib);
                self.flush(cmds);
                self.library = Some(*lib);
                self.publisher.touch();
            }
            Input::StripChanged(cfg) => self.strip_changed(cfg, now_us),
            Input::StripPause(pause) => {
                self.strip_paused = pause;
                if pause {
                    self.strip = StripStatus::Connecting;
                }
                if let Some(tx) = &self.strip_tx {
                    let _ = tx.send(StripMsg::Pause(pause));
                }
                self.publisher.touch();
            }
            Input::Quit => return false,
        }
        self.after(now_us);
        true
    }

    fn identify(&mut self, segment: u8, now_us: u64) {
        let ready = matches!(self.strip, StripStatus::Connected { .. })
            && (1..=self.segments).contains(&segment)
            && self
                .identify_at
                .is_none_or(|t| now_us >= t + IDENTIFY_GATE_US);
        let Some(tx) = self.strip_tx.as_ref().filter(|_| ready) else {
            return;
        };
        let _ = tx.send(StripMsg::Identify(segment));
        self.identify_at = Some(now_us);
    }

    /// Re-pairing: the strip task reconnects; the scene is resent when it reports `Connected`.
    /// A different segment count cannot keep the old plan, so the engine starts over.
    fn strip_changed(&mut self, cfg: StripConfig, now_us: u64) {
        if cfg.segments != self.segments {
            log::warn!(
                "segment count changed from {} to {}",
                self.segments,
                cfg.segments
            );
            self.segments = cfg.segments;
            self.engine = engine_for(&self.settings, cfg.segments, self.last.lights_on);
            if let Some(lib) = &self.library {
                // A fresh engine is not playing: the library (or its fallback) applies at once.
                let cmds = self.engine.set_library(now_us, lib);
                self.flush(cmds);
            }
        }
        self.strip = StripStatus::Connecting;
        if let Some(tx) = &self.strip_tx {
            let _ = tx.send(StripMsg::Configure(cfg));
        }
        self.publisher.touch();
    }

    /// midir keeps its connection object after the `CoreMIDI` endpoint vanishes, so ask the OS.
    /// A stopped clock reconnects anyway: a recreated endpoint leaves a connection that is silent.
    fn check_midi_source(&mut self) {
        let Some(wanted) = self.settings.midi_source.clone() else {
            return;
        };
        self.source_present = self.midi.source_available(&wanted);
        let connected = self.midi.connected_source();
        if !self.source_present
            || !self.last.playing
            || needs_reconnect(Some(&wanted), connected.as_deref())
        {
            self.midi.reconnect(Some(&wanted));
        }
    }

    fn apply_settings(&mut self, new: Settings, now_us: u64) {
        let mut cmds = Vec::new();
        let mut apply = |c: UiCommand| cmds.extend(self.engine.command(now_us, c));
        if new.brightness_pct != self.settings.brightness_pct {
            apply(UiCommand::Brightness(new.brightness_pct));
        }
        if new.phrase_bars != self.settings.phrase_bars {
            apply(UiCommand::Phrase(phrase(new.phrase_bars)));
        }
        if new.lead_ms != self.settings.lead_ms {
            apply(UiCommand::LeadMs(new.lead_ms));
        }
        let source_changed = new.midi_source != self.settings.midi_source;
        if source_changed {
            self.midi.reconnect(new.midi_source.as_deref());
            self.source_present = new
                .midi_source
                .as_deref()
                .is_none_or(|n| self.midi.source_available(n));
        }
        if new.audio_input != self.settings.audio_input && self.capture != Capture::Closed {
            // Reopened on the new input by `sync_capture`.
            self.audio.close();
            self.capture = Capture::Closed;
            self.level_db = None;
        }
        let thresholds = |s: &Settings| (s.sensitivity, s.calm_kick_db, s.calm_high_db, s.riser_db);
        if let (true, Some(d)) = (
            thresholds(&new) != thresholds(&self.settings),
            &mut self.detect,
        ) {
            let p = detector_params(&new);
            d.moment
                .set_thresholds(p.calm_kick_db, p.calm_high_db, p.riser_db);
        }
        self.settings = new;
        self.flush(cmds);
        self.publisher.touch();
    }

    /// The engine's own deadline (clock timeout, next strip message) or the next status update.
    pub fn deadline(&self) -> Option<u64> {
        [
            self.engine.next_deadline_us(),
            self.publisher.deadline(),
            self.tray_due,
            self.identify_at.map(|t| t + IDENTIFY_FLASH_US),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// A deadline passed.
    pub fn tick(&mut self, now_us: u64) {
        self.after(now_us);
    }

    fn after(&mut self, now_us: u64) {
        // Also after every input: a flood of inputs must not starve the clock timeout.
        if self.engine.next_deadline_us().is_some_and(|d| d <= now_us) {
            let cmds = self.engine.on_deadline(now_us);
            self.flush(cmds);
        }
        if self
            .identify_at
            .is_some_and(|t| t + IDENTIFY_FLASH_US <= now_us)
        {
            // The flash overwrote the scene: forget what was sent so the plan repaints all of it.
            self.identify_at = None;
            self.engine.strip_reconnected();
            self.replan(now_us);
        }
        let now = snapshot(&self.engine);
        if now != self.last {
            self.last = now;
            self.publisher.touch();
        }
        self.tell_strip();
        self.sync_capture();
        self.update_tray(now_us);
        if self.publisher.due(now_us) {
            let status = self.status();
            (self.emit)(&status);
        }
    }
}

/// The loop: block for an input or the next deadline.
fn run<M: MidiPort, A: AudioPort, E: FnMut(&Status)>(
    rx: &Receiver<Input>,
    clock: &MonoClock,
    core: &mut Core<M, A, E>,
    visible: &AtomicBool,
) {
    loop {
        core.set_visible(visible.load(Ordering::Relaxed));
        let input = match timeout(core.deadline(), clock.now_us()) {
            None => match rx.recv() {
                Ok(i) => Some(i),
                Err(_) => return,
            },
            Some(wait) => match rx.recv_timeout(wait) {
                Ok(i) => Some(i),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };
        let now = clock.now_us();
        match input {
            Some(i) => {
                if !core.handle(i, now) {
                    return;
                }
            }
            None => core.tick(now),
        }
    }
}

/// Called from the strip task (inside Tokio): it must never block on the engine's queue.
fn report_strip(tx: &SyncSender<Input>, s: StripStatus) {
    // A state change is rare and must not be lost: a full queue hands it to a short-lived thread.
    if let Err(TrySendError::Full(input)) = tx.try_send(Input::StripState(s)) {
        let tx = tx.clone();
        let _ = std::thread::Builder::new()
            .name("strip-report".into())
            .spawn(move || {
                let _ = tx.send(input);
            });
    }
}

pub struct RuntimeHandle {
    pub tx: SyncSender<Input>,
}

/// Starts the engine thread (and the strip task when there is a strip).
pub fn spawn(
    app: AppHandle,
    settings: Settings,
    strip: Option<StripConfig>,
    library: Library,
) -> RuntimeHandle {
    let (tx, rx) = sync_channel(QUEUE);
    let clock = MonoClock::new();
    let segments = strip.as_ref().map_or(DEFAULT_SEGMENTS, |c| c.segments);
    // The strip task always runs: with no strip file it waits for the wizard's config.
    let strip_tx = {
        let (stx, srx) = unbounded_channel();
        let engine_tx = tx.clone();
        strip::spawn(strip, srx, move |s| report_strip(&engine_tx, s));
        Some(stx)
    };
    let midi_tx = tx.clone();
    let midi_watch_tx = tx.clone();
    let visible = app.state::<AppState>().visible_flag();
    let spawned = std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            let midi = MidiIn::start(settings.midi_source.as_deref(), midi_tx, clock);
            let emit = {
                let app = app.clone();
                move |s: &Status| {
                    app.state::<AppState>().set_status(s.clone());
                    for window in ["popover", "editor"] {
                        if let Err(e) = app.emit_to(window, STATUS_EVENT, s) {
                            log::warn!("status event not delivered: {e}");
                        }
                    }
                }
            };
            let mut core = Core::new(
                settings,
                segments,
                strip_tx,
                midi,
                AudioIn::default(),
                emit,
                clock.now_us(),
            );
            let strip_app = app.clone();
            core.set_strip_sink(Box::new(move |s| {
                strip_app.state::<AppState>().set_strip(s.clone());
            }));
            let meter_app = app.clone();
            core.set_meters(Box::new(move |e| {
                if let Err(ee) = meter_app.emit_to("editor", DETECTOR_EVENT, e) {
                    log::debug!("detector event not delivered: {ee}");
                }
            }));
            let list_app = app.clone();
            let choice_app = app.clone();
            core.set_midi_sinks(
                Box::new(move |l| {
                    if let Err(e) = list_app.emit(MIDI_SOURCES_EVENT, l) {
                        log::debug!("midi-sources event not delivered: {e}");
                    }
                }),
                Box::new(move |n| choice_app.state::<AppState>().remember_midi_source(n)),
            );
            crate::midi::watch_sources(&midi_watch_tx);
            core.handle(Input::Library(Box::new(library)), clock.now_us());
            app.state::<AppState>().set_status(core.status());
            let tray_app = app.clone();
            core.set_tray(
                Box::new(move |prev, view| crate::tray::apply(&tray_app, prev, view)),
                clock.now_us(),
            );
            run(&rx, &clock, &mut core, &visible);
        });
    if let Err(e) = spawned {
        log::error!("engine thread not started: {e}");
    }
    RuntimeHandle { tx }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct FakeMidi {
        reconnects: Rc<RefCell<Vec<Option<String>>>>,
        /// What the connection object still holds, like midir after the endpoint vanished.
        present: Option<String>,
        /// What `CoreMIDI` lists.
        ports: Rc<RefCell<Vec<String>>>,
    }

    impl MidiPort for FakeMidi {
        fn connected_source(&self) -> Option<String> {
            self.present.clone()
        }
        fn source_available(&self, name: &str) -> bool {
            self.ports.borrow().iter().any(|p| p == name)
        }
        fn dropped(&self) -> u64 {
            0
        }
        fn reconnect(&mut self, source: Option<&str>) {
            self.reconnects.borrow_mut().push(source.map(String::from));
            self.present = source
                .filter(|n| self.source_available(n))
                .map(String::from);
        }
    }

    #[derive(Default)]
    struct FakeAudio {
        fail: bool,
        opens: Rc<RefCell<u32>>,
        closes: Rc<RefCell<u32>>,
        samples: Vec<f32>,
    }

    impl AudioPort for FakeAudio {
        fn open(&mut self, _device: Option<&str>) -> Result<AudioInfo, AudioError> {
            *self.opens.borrow_mut() += 1;
            if self.fail {
                return Err(AudioError::NoDevice);
            }
            Ok(AudioInfo {
                rate: 48_000,
                name: "Fake mic".into(),
            })
        }
        fn close(&mut self) {
            *self.closes.borrow_mut() += 1;
        }
        fn drain(&mut self, buf: &mut Vec<f32>) {
            buf.append(&mut self.samples);
        }
        fn overflow(&self) -> u64 {
            0
        }
    }

    type TestCore = Core<FakeMidi, FakeAudio, Box<dyn FnMut(&Status)>>;

    fn core(
        strip: bool,
    ) -> (
        TestCore,
        tokio::sync::mpsc::UnboundedReceiver<StripMsg>,
        Rc<RefCell<u32>>,
    ) {
        let (tx, rx) = unbounded_channel();
        let emits = Rc::new(RefCell::new(0));
        let e = Rc::clone(&emits);
        let emit: Box<dyn FnMut(&Status)> = Box::new(move |_: &Status| *e.borrow_mut() += 1);
        let core = Core::new(
            Settings {
                midi_source: Some("Deeboo Clock Sync Out".into()),
                ..Settings::default()
            },
            10,
            strip.then_some(tx),
            FakeMidi::default(),
            FakeAudio::default(),
            emit,
            0,
        );
        (core, rx, emits)
    }

    fn connected() -> StripStatus {
        StripStatus::Connected {
            name: "dj".into(),
            segments: 10,
            reported: 10,
            ip: None,
        }
    }

    #[test]
    fn strip_changes_reach_the_sink_even_while_the_popover_is_hidden() {
        let (mut c, _rx, _) = core(true);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let s = Rc::clone(&seen);
        c.set_visible(false);
        c.set_strip_sink(Box::new(move |st| s.borrow_mut().push(st.clone())));
        c.handle(Input::StripState(connected()), 1_000);
        c.handle(Input::StripState(connected()), 2_000);
        c.handle(Input::StripState(StripStatus::Error), 3_000);
        assert_eq!(
            *seen.borrow(),
            [StripStatus::Connecting, connected(), StripStatus::Error]
        );
    }

    #[test]
    fn deadline_is_none_when_idle() {
        let (mut c, _rx, _) = core(false);
        c.set_visible(false);
        // No strip, nothing scheduled, popover hidden: the thread blocks in `recv()`.
        assert!(c.deadline().is_none());
        assert!(timeout(None, 5).is_none());
        assert_eq!(
            timeout(Some(1_500), 500),
            Some(Duration::from_micros(1_000))
        );
        assert_eq!(timeout(Some(10), 500), Some(Duration::ZERO));
    }

    #[test]
    fn status_emits_only_when_visible_and_coalesced() {
        let (mut c, _rx, emits) = core(false);
        c.set_visible(true);
        // 100 changes in 100 ms.
        for ms in 0..100u64 {
            let cmd = UiCommand::Brightness(10 + u8::try_from(ms % 90).unwrap_or(0));
            assert!(c.handle(Input::Ui(cmd), ms * 1000));
            c.tick(ms * 1000);
        }
        assert!((1..=4).contains(&*emits.borrow()), "{}", emits.borrow());
        // Hidden: changes accumulate, nothing goes out.
        let (mut h, _rx, hidden) = core(false);
        h.set_visible(false);
        for ms in 0..100u64 {
            h.handle(Input::Ui(UiCommand::Force(Moment::Calm)), ms * 1000);
            h.tick(ms * 1000);
        }
        assert_eq!(*hidden.borrow(), 0);
        // Showing sends the latest state right away.
        h.set_visible(true);
        h.handle(Input::PopoverShown, 200_000);
        assert_eq!(*hidden.borrow(), 1);
    }

    #[test]
    fn pending_update_is_flushed_at_its_deadline() {
        let (mut c, _rx, emits) = core(false);
        c.set_visible(true);
        c.handle(Input::PopoverShown, 0);
        c.handle(Input::Ui(UiCommand::Auto), 1_000);
        assert_eq!(*emits.borrow(), 1);
        assert_eq!(c.deadline(), Some(MIN_PUBLISH_GAP_US));
        c.tick(MIN_PUBLISH_GAP_US);
        assert_eq!(*emits.borrow(), 2);
    }

    #[test]
    fn paint_error_marks_strip_and_keeps_engine_running() {
        let (mut c, mut rx, _) = core(true);
        // Connecting: nothing is sent yet.
        assert!(rx.try_recv().is_err());
        c.handle(Input::StripState(connected()), 1_000);
        // The strip gets its first message once the scheduler's gap has passed.
        c.tick(c.deadline().unwrap_or(0));
        assert!(
            matches!(rx.try_recv(), Ok(StripMsg::Paint(_))),
            "rest scene on connect"
        );
        c.handle(Input::StripState(StripStatus::Error), 2_000);
        assert_eq!(c.status().strip, StripStatus::Error);
        // The engine keeps running: clock bytes are handled, nothing reaches the dead strip.
        let midi = |at_us, status| Input::Midi(MidiEvent { at_us, status });
        assert!(c.handle(midi(3_000, 0xFA), 3_000));
        for i in 0..48 {
            let at = 4_000 + i * 20_000;
            assert!(c.handle(midi(at, 0xF8), at));
        }
        assert!(c.status().playing);
        c.tick(2_000_000);
        assert!(rx.try_recv().is_err(), "a message reached the dead strip");
        // Lights on asks the strip task to reconnect.
        c.handle(Input::Ui(UiCommand::Lights(true)), 2_100_000);
        assert!(matches!(rx.try_recv(), Ok(StripMsg::Connect)));
    }

    fn new_cfg(name: &str, segments: u8) -> StripConfig {
        StripConfig {
            id: "bf1234".into(),
            name: name.into(),
            local_key: secrecy::SecretString::from("0123456789abcdef"),
            segments,
            reported: segments,
            last_ip: None,
        }
    }

    /// Ticks through the scheduler's gaps and collects what reaches the strip task.
    fn painted(
        c: &mut TestCore,
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<StripMsg>,
    ) -> Vec<PaintCommand> {
        let mut out = Vec::new();
        for _ in 0..40 {
            while let Ok(m) = rx.try_recv() {
                if let StripMsg::Paint(p) = m {
                    out.push(p);
                }
            }
            let Some(d) = c.deadline() else { break };
            c.tick(d);
        }
        out
    }

    #[test]
    fn library_input_reaches_the_popover_status() {
        let (mut c, _rx, _) = core(false);
        assert_eq!(c.status().selection.set, "Auto");
        let mut lib = Library::builtin(10);
        lib.sets[0].name = "Club".into();
        c.handle(Input::Library(Box::new(lib.clone())), 1_000);
        assert_eq!(c.library, Some(lib));
        assert_eq!(c.status().selection.set, "Club");
    }

    #[test]
    fn auto_palette_shows_the_one_in_force() {
        let (mut c, _rx, _) = core(false);
        let mut lib = Library::builtin(10);
        lib.active_palette = None;
        let in_force = lib.sets[0].palettes[1];
        let name = lib
            .palettes
            .iter()
            .find(|(i, _)| *i == in_force)
            .unwrap()
            .1
            .name
            .clone();
        assert_ne!(
            lib.current_palette().name,
            name,
            "first differs from second"
        );
        c.handle(Input::Library(Box::new(lib)), 1_000);
        c.last.palette = Some(in_force);
        let sel = c.status().selection;
        assert!(sel.palette_auto);
        assert_eq!(sel.palette.name, name);
    }

    #[test]
    fn identify_flashes_one_segment_then_restores_the_scene() {
        let (mut c, mut rx, _) = core(true);
        c.handle(Input::StripState(connected()), 1_000);
        let _ = painted(&mut c, &mut rx);
        c.handle(Input::Identify(3), 2_000_000);
        assert!(matches!(rx.try_recv(), Ok(StripMsg::Identify(3))));
        assert_eq!(c.deadline(), Some(2_000_000 + IDENTIFY_FLASH_US));
        // Too soon after the last one: dropped, the strip loses back-to-back messages.
        c.handle(Input::Identify(4), 2_100_000);
        assert!(rx.try_recv().is_err());
        c.tick(2_000_000 + IDENTIFY_FLASH_US);
        assert!(
            !painted(&mut c, &mut rx).is_empty(),
            "full scene repainted after the flash"
        );
        assert_eq!(c.deadline(), None);
    }

    #[test]
    fn preview_palette_restores_show() {
        use light_core::model::{Palette, Role, Sequence, Step, StepLen};
        use light_core::paint::Hsv;
        let hue = |h| Hsv {
            h,
            s: 1000,
            v: 1000,
        };
        let (mut c, mut rx, _) = core(true);
        c.handle(Input::StripState(connected()), 1_000);
        let _ = painted(&mut c, &mut rx);
        let pal = Palette {
            name: "P".into(),
            principal: hue(111),
            contraste: hue(222),
            suave: hue(333),
        };
        let seq = Sequence {
            name: "S".into(),
            moment: Moment::Rhythm,
            steps: vec![Step {
                len: StepLen::One,
                roles: vec![Role::Principal; 10],
            }],
        };
        let hues = |cmds: &[PaintCommand]| -> Vec<u16> {
            cmds.iter()
                .filter_map(|c| match c {
                    PaintCommand::Bucket(h) | PaintCommand::Pencil { color: h, .. } => Some(h.h),
                    PaintCommand::Eraser { .. } => None,
                })
                .collect()
        };
        c.handle(Input::PreviewPalette(Some(Box::new((pal, seq)))), 2_000_000);
        assert!(hues(&painted(&mut c, &mut rx)).contains(&111));
        c.handle(Input::PreviewPalette(None), 3_000_000);
        let back = hues(&painted(&mut c, &mut rx));
        assert!(!back.is_empty(), "the show is repainted");
        assert!(!back.contains(&111));
        assert_eq!(c.deadline(), None, "the preview stops ticking");
    }

    #[test]
    fn identify_is_ignored_off_strip_or_out_of_range() {
        let (mut c, mut rx, _) = core(true);
        c.handle(Input::Identify(3), 1_000);
        assert!(rx.try_recv().is_err(), "not connected yet");
        c.handle(Input::StripState(connected()), 2_000);
        let _ = painted(&mut c, &mut rx);
        for bad in [0, 11, 255] {
            c.handle(Input::Identify(bad), 3_000_000);
        }
        assert!(rx.try_recv().is_err());
        c.handle(Input::StripPause(true), 4_000_000);
        let _ = rx.try_recv();
        c.handle(Input::Identify(3), 5_000_000);
        assert!(rx.try_recv().is_err(), "paused");
    }

    #[test]
    fn strip_changed_reconnects_and_resends_scene() {
        let (mut c, mut rx, _) = core(true);
        c.handle(Input::StripState(connected()), 1_000);
        assert!(
            !painted(&mut c, &mut rx).is_empty(),
            "scene on first connect"
        );
        c.handle(Input::StripChanged(new_cfg("new", 10)), 2_000_000);
        assert!(matches!(rx.try_recv(), Ok(StripMsg::Configure(_))));
        assert_eq!(c.status().strip, StripStatus::Connecting);
        assert!(
            painted(&mut c, &mut rx).is_empty(),
            "nothing while connecting"
        );
        let again = StripStatus::Connected {
            name: "new".into(),
            segments: 10,
            reported: 10,
            ip: None,
        };
        c.handle(Input::StripState(again), 3_000_000);
        assert!(
            !painted(&mut c, &mut rx).is_empty(),
            "scene resent after reconnect"
        );
    }

    #[test]
    fn pause_lets_the_strip_go_while_the_engine_keeps_running() {
        let (mut c, mut rx, _) = core(true);
        c.handle(Input::StripState(connected()), 1_000);
        let _ = painted(&mut c, &mut rx);
        c.handle(Input::StripPause(true), 2_000_000);
        assert!(matches!(rx.try_recv(), Ok(StripMsg::Pause(true))));
        assert_eq!(c.status().strip, StripStatus::Connecting);
        // The task may report a connection it had before it saw the pause: not believed.
        c.handle(Input::StripState(connected()), 2_100_000);
        assert_eq!(c.status().strip, StripStatus::Connecting);
        c.handle(Input::Ui(UiCommand::Lights(false)), 2_200_000);
        assert!(
            painted(&mut c, &mut rx).is_empty(),
            "nothing is sent while paused"
        );
        assert!(!c.status().lights_on, "the engine took the command");
        c.handle(Input::StripPause(false), 3_000_000);
        assert!(matches!(rx.try_recv(), Ok(StripMsg::Pause(false))));
        c.handle(Input::StripState(connected()), 3_100_000);
        assert!(
            !painted(&mut c, &mut rx).is_empty(),
            "full scene after the strip is back"
        );
    }

    #[test]
    fn strip_changed_with_another_segment_count_rebuilds_the_engine() {
        let (mut c, mut rx, _) = core(true);
        c.handle(Input::StripState(connected()), 1_000);
        let _ = painted(&mut c, &mut rx);
        c.handle(Input::StripChanged(new_cfg("short", 4)), 2_000_000);
        let _ = rx.try_recv();
        let now = StripStatus::Connected {
            name: "short".into(),
            segments: 4,
            reported: 4,
            ip: None,
        };
        c.handle(Input::StripState(now), 3_000_000);
        let cmds = painted(&mut c, &mut rx);
        assert_ne!(cmds.len(), 0);
        assert!(
            cmds.iter().all(|p| light_core::paint::encode(4, p).is_ok()),
            "every command fits 4 segments"
        );
    }

    #[test]
    fn key_invalid_and_not_found_show_and_lights_on_retries_only_the_latter() {
        let (mut c, mut rx, _) = core(true);
        c.handle(Input::StripState(StripStatus::KeyInvalid), 1_000);
        assert_eq!(c.status().strip, StripStatus::KeyInvalid);
        c.handle(Input::Ui(UiCommand::Lights(true)), 2_000);
        assert!(rx.try_recv().is_err(), "a rejected key is not retried");
        c.handle(Input::StripState(StripStatus::NotFound), 3_000);
        c.handle(Input::Ui(UiCommand::Lights(true)), 4_000);
        assert!(matches!(rx.try_recv(), Ok(StripMsg::Connect)));
    }

    const SOURCE: &str = "Deeboo Clock Sync Out";

    #[test]
    fn reconnects_missing_source_on_popover_show() {
        assert!(needs_reconnect(Some("A"), None));
        assert!(needs_reconnect(Some("A"), Some("B")));
        assert!(!needs_reconnect(Some("A"), Some("A")));
        assert!(!needs_reconnect(None, None));
        let (mut c, _rx, _) = core(false);
        let calls = Rc::clone(&c.midi.reconnects);
        let ports = Rc::clone(&c.midi.ports);
        ports.borrow_mut().push(SOURCE.into());
        c.handle(Input::PopoverShown, 0);
        assert_eq!(*calls.borrow(), vec![Some(SOURCE.to_string())]);
        assert!(c.status().midi.ok);
        // Connected and playing: left alone.
        let midi = |at_us, status| Input::Midi(MidiEvent { at_us, status });
        c.handle(midi(1, 0xFA), 1);
        c.handle(midi(2, 0xF8), 2);
        assert!(c.status().playing);
        c.handle(Input::PopoverShown, 3);
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn lost_source_turns_red_and_reconnects_when_back() {
        let (mut c, _rx, _) = core(false);
        let calls = Rc::clone(&c.midi.reconnects);
        let ports = Rc::clone(&c.midi.ports);
        ports.borrow_mut().push(SOURCE.into());
        c.handle(Input::PopoverShown, 0);
        assert!(c.status().midi.ok);
        // The endpoint vanishes; the stale connection object still reports the name.
        ports.borrow_mut().clear();
        assert_eq!(c.midi.connected_source().as_deref(), Some(SOURCE));
        c.handle(Input::PopoverShown, 1);
        assert!(!c.status().midi.ok, "dot red while the source is gone");
        // It comes back: the next show connects again (stopped clock reconnects anyway).
        ports.borrow_mut().push(SOURCE.into());
        c.handle(Input::PopoverShown, 2);
        assert!(c.status().midi.ok);
        assert_eq!(calls.borrow().len(), 3);
    }

    fn sources(names: &[&str]) -> Input {
        Input::SourcesChanged(names.iter().map(|n| (*n).to_string()).collect())
    }

    #[test]
    fn source_appearing_later_is_connected() {
        let (mut c, _rx, _) = core(false);
        let calls = Rc::clone(&c.midi.reconnects);
        let ports = Rc::clone(&c.midi.ports);
        assert!(!c.status().midi.ok);
        ports.borrow_mut().push(SOURCE.into());
        c.handle(sources(&[SOURCE]), 0);
        assert_eq!(*calls.borrow(), vec![Some(SOURCE.to_string())]);
        assert!(c.status().midi.ok);
    }

    #[test]
    fn vanished_source_reconnects_when_back() {
        let (mut c, _rx, _) = core(false);
        let calls = Rc::clone(&c.midi.reconnects);
        let ports = Rc::clone(&c.midi.ports);
        ports.borrow_mut().push(SOURCE.into());
        c.handle(sources(&[SOURCE]), 0);
        ports.borrow_mut().clear();
        c.handle(sources(&[]), 1);
        assert!(!c.status().midi.ok, "red while it is gone");
        ports.borrow_mut().push(SOURCE.into());
        c.handle(sources(&[SOURCE]), 2);
        assert!(c.status().midi.ok);
        assert_eq!(calls.borrow().len(), 2, "connected again once it is back");
    }

    #[test]
    fn no_duplicate_connection_on_repeated_notifications() {
        let (mut c, _rx, _) = core(false);
        let calls = Rc::clone(&c.midi.reconnects);
        c.midi.ports.borrow_mut().push(SOURCE.into());
        for t in 0..3 {
            c.handle(sources(&[SOURCE]), t);
        }
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn single_source_auto_selected_and_remembered() {
        let (mut c, _rx, _) = core(false);
        c.settings.midi_source = None;
        let chosen = Rc::new(RefCell::new(Vec::new()));
        let listed = Rc::new(RefCell::new(Vec::new()));
        let (ch, li) = (Rc::clone(&chosen), Rc::clone(&listed));
        c.set_midi_sinks(
            Box::new(move |l| li.borrow_mut().push(l.to_vec())),
            Box::new(move |n| ch.borrow_mut().push(n.to_string())),
        );
        c.midi.ports.borrow_mut().push("Mixer".into());
        c.handle(sources(&["Mixer", "Other"]), 0);
        assert!(chosen.borrow().is_empty(), "two sources: the user picks");
        c.handle(sources(&["Mixer"]), 1);
        assert_eq!(*chosen.borrow(), vec!["Mixer".to_string()]);
        assert_eq!(c.settings.midi_source.as_deref(), Some("Mixer"));
        assert!(c.status().midi.ok);
        assert_eq!(listed.borrow().len(), 2, "every change reaches the UI");
    }

    #[test]
    fn input_flood_does_not_keep_the_show_from_stopping() {
        let (mut c, _rx, _) = core(false);
        let midi = |at_us, status| Input::Midi(MidiEvent { at_us, status });
        c.handle(midi(0, 0xFA), 0);
        for i in 0..4 {
            c.handle(midi(i * 20_000, 0xF8), i * 20_000);
        }
        assert!(c.status().playing);
        // The clock stops; the loop never times out because inputs keep coming.
        for ms in 100..800u64 {
            c.handle(Input::Ui(UiCommand::Brightness(50)), ms * 1_000);
        }
        assert!(!c.status().playing, "500 ms without clock must stop it");
    }

    #[test]
    fn full_queue_does_not_block_the_strip_report() {
        let (tx, rx) = sync_channel(1);
        tx.send(Input::PopoverShown).unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let t = tx.clone();
        std::thread::spawn(move || {
            report_strip(&t, StripStatus::Error);
            let _ = done_tx.send(());
        });
        assert!(
            done_rx.recv_timeout(Duration::from_millis(500)).is_ok(),
            "report_strip blocked on a full queue"
        );
        // The state change is not lost: it arrives once the engine drains.
        assert!(matches!(rx.recv(), Ok(Input::PopoverShown)));
        assert!(matches!(
            rx.recv_timeout(Duration::from_millis(500)),
            Ok(Input::StripState(StripStatus::Error))
        ));
    }

    #[test]
    fn quit_stops_the_loop() {
        let (mut c, _rx, _) = core(false);
        assert!(!c.handle(Input::Quit, 0));
    }

    fn start(c: &mut TestCore) {
        let midi = |at_us, status| Input::Midi(MidiEvent { at_us, status });
        c.handle(midi(0, 0xFA), 0);
        for i in 1..=48u64 {
            c.handle(midi(i * 20_000, 0xF8), i * 20_000);
        }
    }

    #[test]
    fn capture_only_while_playing_auto_and_lights() {
        for playing in [false, true] {
            for auto in [false, true] {
                for lights in [false, true] {
                    assert_eq!(
                        capture_wanted(playing, auto, lights),
                        playing && auto && lights
                    );
                }
            }
        }
        let (mut c, _rx, _) = core(false);
        let (opens, closes) = (Rc::clone(&c.audio.opens), Rc::clone(&c.audio.closes));
        assert_eq!(*opens.borrow(), 0, "closed while stopped");
        start(&mut c);
        assert_eq!(*opens.borrow(), 1);
        assert!(c.status().audio.ok);
        assert_eq!(c.status().audio.device.as_deref(), Some("Fake mic"));
        c.handle(Input::Ui(UiCommand::Lights(false)), 2_000_000);
        assert_eq!(*closes.borrow(), 1);
        assert!(!c.status().audio.ok);
    }

    #[test]
    fn open_failure_reports_no_audio() {
        let (mut c, _rx, _) = core(false);
        c.audio.fail = true;
        let opens = Rc::clone(&c.audio.opens);
        start(&mut c);
        assert!(!c.status().audio.ok);
        assert_eq!(*opens.borrow(), 1, "no retry storm while playing");
        let mut buf = Vec::new();
        assert_eq!(
            detect_beat(&mut c.audio, &mut None, &mut buf, &mut None, &mut None),
            None
        );
    }

    #[test]
    fn beat_block_feeds_level_and_detector() {
        let (mut c, _rx, _) = core(false);
        start(&mut c);
        c.audio.samples = vec![0.5; 48_000];
        for i in 49..=72u64 {
            let at = i * 20_000;
            c.handle(
                Input::Midi(MidiEvent {
                    at_us: at,
                    status: 0xF8,
                }),
                at,
            );
        }
        assert_eq!(c.level_db, Some(-6.0));
        assert_eq!(c.status().audio.level_db, Some(-6.0));
    }

    /// One MIDI clock tick per `step_us` with a loud block of audio before each; returns the events.
    fn run_ticks(c: &mut TestCore, ticks: std::ops::Range<u64>, step_us: u64) {
        for i in ticks {
            let at = i * step_us;
            c.audio.samples = vec![0.5; 4_800];
            c.handle(
                Input::Midi(MidiEvent {
                    at_us: at,
                    status: 0xF8,
                }),
                at,
            );
        }
    }

    fn meter_core() -> (TestCore, Rc<RefCell<Vec<DetectorEvent>>>) {
        let (mut c, _rx, _) = core(false);
        start(&mut c);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let s = Rc::clone(&seen);
        c.set_meters(Box::new(move |e| s.borrow_mut().push(e.clone())));
        (c, seen)
    }

    #[test]
    fn meters_emit_only_while_detection_open() {
        let (mut c, seen) = meter_core();
        run_ticks(&mut c, 49..121, 20_000);
        assert!(seen.borrow().is_empty(), "not watching: zero events");
        c.handle(Input::DetectionWatch(true), 2_500_000);
        run_ticks(&mut c, 121..169, 20_000);
        let n = seen.borrow().len();
        assert!(n >= 1, "watching: one event per beat");
        let e = seen.borrow()[0].clone();
        assert!(e.kick_db.is_finite() && e.kick_ref_db.is_finite());
        c.handle(Input::DetectionWatch(false), 3_500_000);
        run_ticks(&mut c, 169..241, 20_000);
        assert_eq!(seen.borrow().len(), n, "closed: zero events");
    }

    #[test]
    fn meters_are_capped_at_15_hz() {
        let (mut c, seen) = meter_core();
        c.handle(Input::DetectionWatch(true), 1_000_000);
        // Beats 24 ms apart (a 1 ms tick) for about one second.
        run_ticks(&mut c, 1_000..2_000, 1_000);
        let n = seen.borrow().len();
        assert!((10..=16).contains(&n), "{n}");
    }

    #[test]
    fn sensitivity_change_keeps_the_detector() {
        let (mut c, _rx, _) = core(false);
        start(&mut c);
        let before = c.detect.as_ref().map(|d| d.rate);
        assert_eq!(before, Some(48_000));
        let new = Settings {
            sensitivity: 9,
            ..c.settings.clone()
        };
        c.handle(Input::Settings(new), 3_000_000);
        assert!(c.detect.is_some());
    }

    #[test]
    fn tray_repaints_only_on_change_and_has_no_deadline_when_stopped() {
        let (mut c, _rx, _) = core(true);
        let paints = Rc::new(RefCell::new(0));
        let p = Rc::clone(&paints);
        c.set_tray(Box::new(move |_, _| *p.borrow_mut() += 1), 0);
        assert_eq!(*paints.borrow(), 1, "first paint");
        assert_eq!(c.deadline(), None);
        c.handle(Input::PopoverShown, 10);
        assert_eq!(*paints.borrow(), 1, "nothing changed");
        c.handle(Input::StripState(connected()), 20);
        assert_eq!(*paints.borrow(), 1, "MIDI source missing: still red");
        c.handle(
            Input::Settings(Settings {
                midi_source: None,
                ..Settings::default()
            }),
            30,
        );
        assert_eq!(*paints.borrow(), 2, "dot went green");
        assert_eq!(c.tray_due, None, "stopped: no tray deadline");
    }
}
