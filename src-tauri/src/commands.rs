//! IPC commands. Every DTO here is mirrored in `ui/src/api.ts`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TrySendError;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use light_core::detect::Moment;
use light_core::engine::UiCommand;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::runtime::{Input, RuntimeHandle};

use crate::library_store;
use crate::settings::{self, Settings};
use crate::strip_store;
use crate::wizard::session::{Emit, RealLan, Wizard};
use light_core::library::Library;
use light_core::model::{Sequence, resolve};
use light_core::scene;

/// The most the app waits at exit for the wizard to revoke its cloud session: the request already in
/// flight (10 s timeout) plus the revoke (10 s). It returns as soon as the run ends, and at once
/// when no wizard runs, so only a hanging network makes quitting slow.
const QUIT_WAIT: std::time::Duration = std::time::Duration::from_secs(22);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StripStatus {
    NotConfigured,
    Connecting,
    Connected {
        name: String,
        /// Wired segments.
        segments: u8,
        /// What the controller reports (at least `segments`).
        reported: u8,
        /// The strip's LAN address (private ranges only), when known.
        #[serde(skip_serializing_if = "Option::is_none")]
        ip: Option<std::net::Ipv4Addr>,
    },
    Error,
    /// The strip refused the saved key (re-added in Smart Life): pair again.
    KeyInvalid,
    /// Discovery did not see the strip's ID.
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MidiStatus {
    pub source: Option<String>,
    pub ok: bool,
}

/// Live audio: open only while playing with Auto and Lights on.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Peak of the last beat in dBFS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level_db: Option<f32>,
}

/// The look the popover names: the active set and the palette in force.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub set: String,
    /// The palette follows the set's rotation (nothing pinned).
    pub palette_auto: bool,
    pub palette: PaletteView,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteView {
    pub name: String,
    /// Principal, Contraste, Suave.
    pub colours: [light_core::paint::Hsv; 3],
}

impl Selection {
    pub fn of(lib: &Library) -> Self {
        Self::in_force(lib, None)
    }

    /// `in_force`: the palette the engine plays (library id), which under Auto is the rotation's.
    pub fn in_force(lib: &Library, in_force: Option<u32>) -> Self {
        let p = in_force
            .and_then(|id| lib.palettes.iter().find(|(i, _)| *i == id))
            .map_or_else(|| lib.current_palette(), |(_, p)| p.clone());
        Self {
            set: lib
                .current_set()
                .map_or_else(String::new, |s| s.name.clone()),
            palette_auto: lib.active_palette.is_none(),
            palette: PaletteView {
                name: p.name,
                colours: [p.principal, p.contraste, p.suave],
            },
        }
    }
}

/// Which list `select` steps through.
#[derive(Clone, Copy)]
pub enum Pick {
    Set,
    Palette,
}

/// What the popover shows; the engine thread publishes it only while the popover is visible.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)] // mirrors the UI's status object
pub struct Status {
    pub strip: StripStatus,
    pub bpm: Option<f32>,
    pub playing: bool,
    pub beat_in_bar: Option<u8>,
    pub bar_in_phrase: Option<u8>,
    pub phrase_bars: u8,
    pub moment: MomentDto,
    pub auto: bool,
    pub predicted: bool,
    pub lights_on: bool,
    pub midi: MidiStatus,
    pub audio: AudioStatus,
    pub selection: Selection,
    /// Which entry of the live set's list plays now (the Sets tab highlights it).
    pub clip: Option<ClipDto>,
}

/// `PlayingClip` for the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDto {
    pub moment: MomentDto,
    pub index: usize,
    pub elapsed: u32,
    pub total: u32,
}

impl Status {
    /// Before the engine thread has said anything.
    fn initial() -> Self {
        Self {
            strip: StripStatus::NotConfigured,
            bpm: None,
            playing: false,
            beat_in_bar: None,
            bar_in_phrase: None,
            phrase_bars: 8,
            moment: MomentDto::Rhythm,
            auto: true,
            predicted: false,
            lights_on: true,
            midi: MidiStatus {
                source: None,
                ok: true,
            },
            audio: AudioStatus::default(),
            selection: Selection::of(&Library::builtin(10)),
            clip: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MomentDto {
    Rhythm,
    Calm,
    BuildUp,
}

impl From<Moment> for MomentDto {
    fn from(m: Moment) -> Self {
        match m {
            Moment::Rhythm => Self::Rhythm,
            Moment::Calm => Self::Calm,
            Moment::BuildUp => Self::BuildUp,
        }
    }
}

/// What the popover can ask of the show; anything else is rejected by serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiCommandDto {
    Lights { on: bool },
    Force { moment: MomentDto },
    Auto,
}

impl From<UiCommandDto> for UiCommand {
    fn from(c: UiCommandDto) -> Self {
        match c {
            UiCommandDto::Lights { on } => UiCommand::Lights(on),
            UiCommandDto::Force { moment } => UiCommand::Force(match moment {
                MomentDto::Rhythm => Moment::Rhythm,
                MomentDto::Calm => Moment::Calm,
                MomentDto::BuildUp => Moment::BuildUp,
            }),
            UiCommandDto::Auto => UiCommand::Auto,
        }
    }
}

/// Mutable app state shared with the commands.
pub struct AppState {
    /// The last status the engine thread published.
    status: Mutex<Status>,
    settings: Mutex<Settings>,
    /// Where the settings file lives; set at startup.
    settings_path: Mutex<Option<std::path::PathBuf>>,
    library: Mutex<Library>,
    /// `library.json`; set at startup.
    library_path: Mutex<Option<std::path::PathBuf>>,
    runtime: Mutex<Option<RuntimeHandle>>,
    /// Read by the engine thread: status goes out only while this is true (popover or editor shows).
    visible: Arc<AtomicBool>,
    popover_shown: AtomicBool,
    editor_shown: AtomicBool,
    wizard: Wizard,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: Mutex::new(Status::initial()),
            settings: Mutex::new(Settings::default()),
            settings_path: Mutex::new(None),
            library: Mutex::new(Library::builtin(10)),
            library_path: Mutex::new(None),
            runtime: Mutex::new(None),
            visible: Arc::new(AtomicBool::new(false)),
            popover_shown: AtomicBool::new(false),
            editor_shown: AtomicBool::new(false),
            wizard: Wizard::default(),
        }
    }
}

/// The live set exists (else the first) and the pinned palette belongs to it (else Auto).
fn keep_live_valid(lib: &mut Library) {
    if lib.current_set().is_none()
        && let Some(first) = lib.sets.first()
    {
        lib.active_set = first.id;
    }
    let pin = lib.active_palette;
    if pin.is_some_and(|p| lib.current_set().is_none_or(|s| !s.palettes.contains(&p))) {
        lib.active_palette = None;
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    /// Loads the settings file (defaults on any problem) and remembers its path.
    pub fn init_settings(&self, path: std::path::PathBuf) {
        *lock(&self.settings) = settings::load(&path);
        *lock(&self.settings_path) = Some(path);
    }

    /// Loads the library file (built-ins for `segments` on any problem) and remembers its path.
    pub fn init_library(&self, path: std::path::PathBuf, segments: u8) {
        let phrase_bars = lock(&self.settings).phrase_bars;
        *lock(&self.library) = library_store::load(&path, segments, phrase_bars);
        *lock(&self.library_path) = Some(path);
    }

    /// A library that was never saved is just the first-run built-ins: rebuild it for the strip's new
    /// segment count. A saved one is the user's and stays as it is (the editor adapts it).
    fn rebuild_unsaved_library(&self, segments: u8) -> Option<Library> {
        let path = lock(&self.library_path).clone()?;
        let mut current = lock(&self.library);
        if path.exists() || current.segments() == segments {
            return None;
        }
        *current = Library::builtin(segments);
        Some(current.clone())
    }

    pub fn library(&self) -> Library {
        lock(&self.library).clone()
    }

    /// Steps the active set or palette by one, wrapping, and saves it like an editor save. One lock
    /// from read to save: a concurrent editor save cannot be overwritten with a stale copy.
    pub fn select(&self, pick: Pick, delta: i8) -> Result<Selection, String> {
        if delta != 1 && delta != -1 {
            return Err("delta must be -1 or 1".into());
        }
        let mut current = lock(&self.library);
        let mut lib = current.clone();
        let step = |len: usize, at: usize| {
            if delta > 0 {
                (at + 1) % len
            } else {
                (at + len - 1) % len
            }
        };
        match pick {
            Pick::Set => {
                let at = lib
                    .sets
                    .iter()
                    .position(|s| s.id == lib.active_set)
                    .unwrap_or(0);
                lib.active_set = lib.sets[step(lib.sets.len().max(1), at)..]
                    .first()
                    .ok_or("no sets")?
                    .id;
                keep_live_valid(&mut lib);
            }
            Pick::Palette => {
                // Auto sits at both ends of the wrap: Auto, p1 .. pn, Auto.
                let mut ring = vec![None];
                ring.extend(
                    lib.current_set()
                        .into_iter()
                        .flat_map(|s| &s.palettes)
                        .map(|p| Some(*p)),
                );
                let at = ring
                    .iter()
                    .position(|p| *p == lib.active_palette)
                    .unwrap_or(0);
                lib.active_palette = ring[step(ring.len(), at)];
            }
        }
        self.save_into(&mut current, lib).map(|l| Selection::of(&l))
    }

    /// Makes `id` the live set (Sets button); a pin the new set does not hold goes back to Auto.
    pub fn set_live_set(&self, id: u32) -> Result<Selection, String> {
        let mut current = lock(&self.library);
        if !current.sets.iter().any(|s| s.id == id) {
            return Err("No such set.".into());
        }
        let mut lib = current.clone();
        lib.active_set = id;
        keep_live_valid(&mut lib);
        self.save_into(&mut current, lib).map(|l| Selection::of(&l))
    }

    /// Validates, assigns ids to new entries, persists, then swaps. Any failure keeps the previous
    /// library on disk and in memory; messages never echo the input.
    fn save_library(&self, mut new: Library) -> Result<Library, String> {
        let mut current = lock(&self.library);
        // The editor never moves the live selection; only the popover does.
        new.active_set = current.active_set;
        new.active_palette = current.active_palette;
        keep_live_valid(&mut new);
        self.save_into(&mut current, new)
    }

    fn save_into(&self, current: &mut Library, mut new: Library) -> Result<Library, String> {
        let path = lock(&self.library_path)
            .clone()
            .ok_or("library unavailable")?;
        // A re-paired strip with another segment count: the sequences follow the new shape.
        if new.segments() != current.segments() {
            new.resize_segments(new.segments());
        }
        new.validate().map_err(|e| e.to_string())?;
        library_store::assign_ids(current, &mut new).map_err(str::to_string)?;
        library_store::save(&path, &new).map_err(|e| {
            log::warn!("library not saved: {e}");
            "library not saved".to_string()
        })?;
        *current = new.clone();
        Ok(new)
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    fn save_settings(&self, new: Settings) -> Result<Settings, String> {
        new.validate()?;
        let path = lock(&self.settings_path)
            .clone()
            .ok_or("settings unavailable")?;
        settings::save(&path, &new).map_err(|e| {
            log::warn!("settings not saved: {e}");
            "settings not saved".to_string()
        })?;
        *lock(&self.settings) = new.clone();
        Ok(new)
    }

    /// The engine picked the only MIDI source by itself: keep it (a failed save is logged by
    /// `save_settings`; the choice still holds until restart).
    pub fn remember_midi_source(&self, name: &str) {
        let mut s = self.settings();
        s.midi_source = Some(name.to_string());
        let _ = self.save_settings(s);
    }

    /// Saves the Detection panel's thresholds (range-checked by `Settings::validate`); the rest of
    /// the settings stay as they are.
    fn set_thresholds(&self, t: Thresholds) -> Result<Settings, String> {
        let mut s = self.settings();
        (s.calm_kick_db, s.calm_high_db, s.riser_db) = (t.calm_kick_db, t.calm_high_db, t.riser_db);
        self.save_settings(s)
    }

    pub fn set_status(&self, s: Status) {
        *lock(&self.status) = s;
    }

    /// The strip state alone, kept current while the popover is hidden (status is published only
    /// while it shows); the editor decides pairing vs shape from it.
    pub fn set_strip(&self, strip: StripStatus) {
        lock(&self.status).strip = strip;
    }

    pub fn visible_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.visible)
    }

    pub fn set_runtime(&self, handle: RuntimeHandle) {
        *lock(&self.runtime) = Some(handle);
    }

    /// Queues an input for the engine thread; never blocks.
    fn send(&self, input: Input) -> Result<(), String> {
        let guard = lock(&self.runtime);
        let handle = guard.as_ref().ok_or("engine not running")?;
        handle.tx.try_send(input).map_err(|e| match e {
            TrySendError::Full(_) => "engine busy".to_string(),
            TrySendError::Disconnected(_) => "engine stopped".to_string(),
        })
    }

    fn send_strip_pause(&self, pause: bool) {
        self.deliver(Input::StripPause(pause));
    }

    /// For news that must not be lost (a pause that got lost would leave the strip offline; a saved
    /// library the engine never hears of; meters that never stop): a full queue hands the input to
    /// a short-lived thread, like `report_strip`. Never blocks, never fails.
    fn deliver(&self, input: Input) {
        let Some(tx) = lock(&self.runtime).as_ref().map(|h| h.tx.clone()) else {
            return;
        };
        if let Err(TrySendError::Full(input)) = tx.try_send(input) {
            let _ = std::thread::Builder::new()
                .name("engine-news".into())
                .spawn(move || {
                    let _ = tx.send(input);
                });
        }
    }

    /// Called by the tray when the popover shows or hides.
    pub fn set_popover_visible(&self, visible: bool) {
        self.popover_shown.store(visible, Ordering::Relaxed);
        self.refresh_visible();
    }

    /// Called when the editor window opens or closes: it shows the live readout.
    pub fn set_editor_visible(&self, visible: bool) {
        self.editor_shown.store(visible, Ordering::Relaxed);
        self.refresh_visible();
    }

    fn refresh_visible(&self) {
        let any =
            self.popover_shown.load(Ordering::Relaxed) || self.editor_shown.load(Ordering::Relaxed);
        self.visible.store(any, Ordering::Relaxed);
        if any && self.send(Input::PopoverShown).is_err() {
            log::debug!("a window shown but the engine did not get the news");
        }
    }

    /// `strip.json` next to the settings file.
    fn strip_path(&self) -> Result<std::path::PathBuf, String> {
        let path = lock(&self.settings_path).clone();
        path.and_then(|p| p.parent().map(|d| d.join("strip.json")))
            .ok_or_else(|| "settings unavailable".to_string())
    }

    /// Editor window closed: revoke and forget whatever the wizard holds.
    pub fn editor_closed(&self, emit: &Emit) {
        self.wizard.cancel(emit);
        self.deliver(Input::DetectionWatch(false));
        self.deliver(Input::PreviewPalette(None));
        self.deliver(Input::Calibrate(false));
    }

    /// App exit: a running wizard is cancelled (its cloud session revoked) before the engine stops.
    pub fn quit(&self, emit: &Emit) {
        self.wizard.shutdown(emit, QUIT_WAIT);
        let _ = self.send(Input::Quit);
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_status(state: tauri::State<'_, AppState>) -> Status {
    lock(&state.status).clone()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn save_settings(
    state: tauri::State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let saved = state.save_settings(settings)?;
    state.send(Input::Settings(saved.clone()))?;
    Ok(saved)
}

/// The Detection panel's sliders (whole dB); serde rejects anything that is not an integer.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // the wire names are the settings keys
pub struct Thresholds {
    calm_kick_db: u8,
    calm_high_db: u8,
    riser_db: u8,
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn set_detection_thresholds(
    state: tauri::State<'_, AppState>,
    thresholds: Thresholds,
) -> Result<Settings, String> {
    let saved = state.set_thresholds(thresholds)?;
    state.send(Input::Settings(saved.clone()))?;
    Ok(saved)
}

/// The editor's Detection section is open (`true`) or closed: meters flow only while open.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn detection_watch(state: tauri::State<'_, AppState>, on: bool) -> Result<(), String> {
    state.send(Input::DetectionWatch(on))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_library(state: tauri::State<'_, AppState>) -> Library {
    state.library()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn select_set(state: tauri::State<'_, AppState>, delta: i8) -> Result<Selection, String> {
    select(&state, Pick::Set, delta)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn select_palette(state: tauri::State<'_, AppState>, delta: i8) -> Result<Selection, String> {
    select(&state, Pick::Palette, delta)
}

fn select(state: &AppState, pick: Pick, delta: i8) -> Result<Selection, String> {
    let sel = state.select(pick, delta)?;
    // Saved already: an engine that cannot take the news right now gets it a moment later.
    state.deliver(Input::Library(Box::new(state.library())));
    Ok(sel)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn set_live_set(state: tauri::State<'_, AppState>, id: u32) -> Result<Selection, String> {
    let sel = state.set_live_set(id)?;
    state.deliver(Input::Library(Box::new(state.library())));
    Ok(sel)
}

fn save_to(state: &AppState, library: Library) -> Result<Library, String> {
    let saved = state.save_library(library)?;
    state.deliver(Input::Library(Box::new(saved.clone())));
    Ok(saved)
}

/// Entries whose id the stored library lacks are new; the saved library (with real ids) is returned.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn save_library(
    state: tauri::State<'_, AppState>,
    library: Library,
) -> Result<Library, String> {
    save_to(&state, library)
}

/// Messages each step costs to reach from the previous one (the first from the last: it wraps),
/// so the editor can flag steps the strip would show in several parts. Messages never echo input.
fn message_counts_of(lib: &Library, seq: &Sequence, palette_id: u32) -> Result<Vec<u8>, String> {
    if seq.steps.len() > light_core::library::MAX_STEPS {
        return Err("A sequence has at most 128 steps.".into());
    }
    seq.validate(lib.segments())
        .map_err(|_| "Every step needs one role per segment.".to_string())?;
    let (_, palette) = lib
        .palettes
        .iter()
        .find(|(id, _)| *id == palette_id)
        .ok_or("No such palette.")?;
    let scenes: Vec<Vec<_>> = seq
        .steps
        .iter()
        .map(|s| s.roles.iter().map(|r| resolve(*r, palette, 100)).collect())
        .collect();
    let n = scenes.len();
    Ok((0..n)
        .map(|i| {
            let count = scene::diff(&scenes[(i + n - 1) % n], &scenes[i]).len();
            u8::try_from(count).unwrap_or(u8::MAX)
        })
        .collect())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn message_counts(
    state: tauri::State<'_, AppState>,
    sequence: Sequence,
    palette_id: u32,
) -> Result<Vec<u8>, String> {
    message_counts_of(&state.library(), &sequence, palette_id)
}

/// One example of `pattern` with random parameters, for the library's own segment count and shape
/// (every sequence in the library matches it; no strip needed).
fn pattern_sequence_of(
    lib: &Library,
    pattern: light_core::patterns::Pattern,
    seed: u64,
) -> Result<Sequence, String> {
    let n = lib.segments();
    let groups = light_core::groups::groups(&lib.shape);
    let seq = light_core::generator::Generator::new(seed).pattern(pattern, &groups, n);
    seq.validate(n)
        .map_err(|_| "Could not build that pattern for this library.".to_string())?;
    Ok(seq)
}

/// A named pattern as an editable sequence ("Duplicate to edit") or a read-only preview.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn pattern_sequence(
    state: tauri::State<'_, AppState>,
    pattern: light_core::patterns::Pattern,
    seed: u64,
) -> Result<Sequence, String> {
    pattern_sequence_of(&state.library(), pattern, seed)
}

/// One example of what Generated would play for `moment`, built for the saved shape and a fresh seed.
fn preview_generated_of(lib: &Library, moment: Moment, seed: u64) -> Result<Sequence, String> {
    let n = lib.segments();
    let groups = light_core::groups::groups(&lib.shape);
    let seq = light_core::generator::Generator::new(seed).next(moment, &groups, n);
    seq.validate(n)
        .map_err(|_| "Could not generate a sequence for this strip.".to_string())?;
    Ok(seq)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn preview_generated(
    state: tauri::State<'_, AppState>,
    moment: MomentDto,
) -> Result<Sequence, String> {
    let moment = match moment {
        MomentDto::Rhythm => Moment::Rhythm,
        MomentDto::Calm => Moment::Calm,
        MomentDto::BuildUp => Moment::BuildUp,
    };
    // Wall-clock microseconds: any seed works, it only has to differ between clicks.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| u64::try_from(d.as_micros()).unwrap_or(1));
    preview_generated_of(&state.library(), moment, seed)
}

/// What "Show on strip" plays: the palette as sent (range-checked) with the library's own sequence,
/// which must fit the library's segments.
fn preview_palette_of(
    lib: &Library,
    palette: light_core::model::Palette,
    sequence: u32,
) -> Result<Box<(light_core::model::Palette, Sequence)>, String> {
    light_core::library::validate_palette(&palette).map_err(|e| e.to_string())?;
    let seq = lib
        .sequences
        .iter()
        .find(|(id, _)| *id == sequence)
        .map(|(_, s)| s.clone())
        .ok_or("No such sequence.")?;
    seq.validate(lib.segments())
        .map_err(|_| "That sequence does not fit the strip.")?;
    Ok(Box::new((palette, seq)))
}

/// Shows `sequence` in `palette` on the strip (`on`), or hands the strip back to the show.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn preview_palette(
    state: tauri::State<'_, AppState>,
    on: bool,
    palette: light_core::model::Palette,
    sequence: u32,
) -> Result<(), String> {
    let p = if on {
        Some(preview_palette_of(&state.library(), palette, sequence)?)
    } else {
        None
    };
    state.send(Input::PreviewPalette(p))
}

/// Setup's "Calibrate": while `on` and the clock plays, the strip flashes white on every beat instead of
/// the show. Never persisted; the UI, a clock Stop and closing the editor all switch it off.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn calibrate_latency(state: tauri::State<'_, AppState>, on: bool) -> Result<(), String> {
    state.send(Input::Calibrate(on))
}

/// What holding a set clip plays: the palette as sent (range-checked) with the clip's sequence, a
/// pattern or a generated one, built for the saved shape; it must fit the strip.
fn preview_sequence_of(
    lib: &Library,
    palette: light_core::model::Palette,
    clip: light_core::library::SeqRef,
    moment: Moment,
    seed: u64,
) -> Result<Box<(light_core::model::Palette, Sequence)>, String> {
    use light_core::library::SeqRef;
    light_core::library::validate_palette(&palette).map_err(|e| e.to_string())?;
    let seq = match clip {
        SeqRef::Id(id) => return preview_palette_of(lib, palette, id),
        SeqRef::Pattern(p) => pattern_sequence_of(lib, p, seed)?,
        SeqRef::Generated => preview_generated_of(lib, moment, seed)?,
    };
    Ok(Box::new((palette, seq)))
}

/// Hold-to-preview in Sets: plays one clip in `palette` on the strip (`on`), or hands the strip back.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn preview_sequence(
    state: tauri::State<'_, AppState>,
    on: bool,
    palette: light_core::model::Palette,
    sequence: light_core::library::SeqRef,
    moment: MomentDto,
) -> Result<(), String> {
    let p = if on {
        let moment = match moment {
            MomentDto::Rhythm => Moment::Rhythm,
            MomentDto::Calm => Moment::Calm,
            MomentDto::BuildUp => Moment::BuildUp,
        };
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| u64::try_from(d.as_micros()).unwrap_or(1));
        Some(preview_sequence_of(
            &state.library(),
            palette,
            sequence,
            moment,
            seed,
        )?)
    } else {
        None
    };
    state.send(Input::PreviewPalette(p))
}

#[tauri::command]
pub async fn list_midi_sources() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(crate::devices::midi_sources)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_audio_inputs() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(crate::devices::audio_inputs)
        .await
        .map_err(|e| e.to_string())
}

/// Validated by serde on the way in. The lights switch is also persisted.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn ui_command(state: tauri::State<'_, AppState>, cmd: UiCommandDto) -> Result<(), String> {
    if let UiCommandDto::Lights { on } = cmd {
        let mut s = state.settings();
        s.lights_on = on;
        state.save_settings(s)?;
    }
    state.send(Input::Ui(cmd.into()))
}

/// Pushes wizard states to the editor window only.
pub fn wizard_emit(app: &tauri::AppHandle) -> Emit {
    let app = app.clone();
    Arc::new(move |state| {
        if let Err(e) = app.emit_to("editor", "wizard", &state) {
            log::debug!("wizard event not delivered: {e}");
        }
    })
}

/// The User Code is validated here (`UserCode::parse`) and never logged or echoed.
#[tauri::command]
pub async fn wizard_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    user_code: String,
) -> Result<(), String> {
    let code = tuya_cloud::UserCode::parse(&user_code).map_err(|_| {
        "Enter the User Code from Smart Life (letters and digits only).".to_string()
    })?;
    let handle = app.clone();
    let pause: Arc<dyn Fn(bool) + Send + Sync> =
        Arc::new(move |p| handle.state::<AppState>().send_strip_pause(p));
    state.wizard.start(
        Arc::new(RealLan(pause)),
        Arc::new(tuya_cloud::UreqTransport::new()),
        code,
        wizard_emit(&app),
    )
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri deserialises owned arguments
pub fn wizard_choose(state: tauri::State<'_, AppState>, device_id: String) -> Result<(), String> {
    state.wizard.choose(&device_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn wizard_retry(app: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    state.wizard.retry(&wizard_emit(&app));
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn wizard_cancel(app: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    state.wizard.cancel(&wizard_emit(&app));
}

/// `segment` is 1-based and checked against the strip's segment count.
#[tauri::command]
pub async fn strip_identify(state: tauri::State<'_, AppState>, segment: u8) -> Result<(), String> {
    state.wizard.identify(segment).await
}

/// The strip must be connected and `segment` (1-based) one of its segments.
fn identify_check(strip: &StripStatus, segment: u8) -> Result<(), String> {
    match strip {
        StripStatus::Connected { segments, .. } if (1..=*segments).contains(&segment) => Ok(()),
        StripStatus::Connected { .. } => Err("No such segment.".into()),
        _ => Err("Strip not connected.".into()),
    }
}

/// Flashes one segment through the app's own strip connection (the shape editor, after pairing).
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn identify_segment(state: tauri::State<'_, AppState>, segment: u8) -> Result<(), String> {
    identify_check(&lock(&state.status).strip, segment)?;
    state.send(Input::Identify(segment))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn wizard_save(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    wired: u8,
) -> Result<(), String> {
    // `Saved` goes out only once the engine has the new strip; if it cannot take the news now, the
    // file is safe on disk and a restart still picks it up.
    state
        .wizard
        .save(&state.strip_path()?, &wizard_emit(&app), wired, |cfg| {
            if let Some(lib) = state.rebuild_unsaved_library(cfg.segments) {
                state.deliver(Input::Library(Box::new(lib)));
            }
            state.send(Input::StripChanged(cfg)).map_err(|e| {
                log::warn!("strip saved but not reloaded: {e}");
                "Saved. Restart the app to use the new strip.".to_string()
            })
        })
}

/// The shape with exactly `n` pieces: the first ones stay; new ones continue from the last piece in
/// its direction (one cell per step). shortcut: clamps at the grid edge instead of wrapping a row.
fn with_wired(lib: &Library, n: u8) -> Library {
    use light_core::library::{GRID, Piece};
    let mut out = lib.clone();
    out.shape.pieces.truncate(usize::from(n));
    while out.shape.pieces.len() < usize::from(n) {
        let last = out.shape.pieces.last().copied().unwrap_or(Piece {
            x: 0.0,
            y: 0.0,
            angle: 0,
            len: 1,
        });
        let (dx, dy): (f32, f32) = match last.angle {
            90 => (0.0, 1.0),
            180 => (-1.0, 0.0),
            270 => (0.0, -1.0),
            0 => (1.0, 0.0),
            a => {
                let (sin, cos) = f32::from(a).to_radians().sin_cos();
                (cos, sin)
            }
        };
        let step = f32::from(last.len.max(1));
        out.shape.pieces.push(Piece {
            x: dx.mul_add(step, last.x).clamp(0.0, GRID),
            y: dy.mul_add(step, last.y).clamp(0.0, GRID),
            ..last
        });
    }
    out
}

impl AppState {
    /// Saves the wired count in `strip.json` (mode and key untouched), then resizes the shape and the
    /// sequences like a re-pair does (`save_library`).
    fn set_wired(&self, n: u8) -> Result<Library, String> {
        let path = self.strip_path()?;
        let mut cfg = strip_store::load(&path).map_err(|_| "No strip saved yet.".to_string())?;
        if !(1..=cfg.reported).contains(&n) {
            return Err("Wired segments must be between 1 and what the strip reports.".into());
        }
        cfg.segments = n;
        strip_store::save(&path, &cfg).map_err(|e| {
            log::warn!("strip file not saved: {e}");
            "Strip not saved.".to_string()
        })?;
        let lib = self.save_library(with_wired(&self.library(), n))?;
        self.deliver(Input::StripChanged(cfg));
        Ok(lib)
    }
}

/// How many segments are wired (the strip may report more). Returns the resized library.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn set_wired_segments(state: tauri::State<'_, AppState>, n: u8) -> Result<Library, String> {
    let lib = state.set_wired(n)?;
    state.deliver(Input::Library(Box::new(lib.clone())));
    Ok(lib)
}

/// The settings view's Quit button; `RunEvent::Exit` then stops the engine.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_palette_validates_everything() {
        use light_core::paint::Hsv;
        let lib = Library::builtin(10);
        let id = lib.sequences[0].0;
        let pal = lib.palettes[0].1.clone();
        let (p, s) = *preview_palette_of(&lib, pal.clone(), id).unwrap();
        assert_eq!((p, s.name), (pal.clone(), lib.sequences[0].1.name.clone()));
        assert!(preview_palette_of(&lib, pal.clone(), 999_999).is_err());
        let mut bad = pal.clone();
        bad.principal = Hsv { h: 361, s: 0, v: 0 };
        assert!(preview_palette_of(&lib, bad, id).is_err());
        let mut short = lib.clone();
        short.sequences[0].1.steps[0].roles.pop();
        assert!(preview_palette_of(&short, pal, id).is_err());
    }

    #[test]
    fn preview_sequence_resolves_every_kind_of_clip() {
        use light_core::library::SeqRef;
        use light_core::paint::Hsv;
        let lib = Library::builtin(10);
        let id = lib.sequences[0].0;
        let pal = lib.palettes[0].1.clone();
        let m = Moment::Rhythm;
        let (_, s) = *preview_sequence_of(&lib, pal.clone(), SeqRef::Id(id), m, 1).unwrap();
        assert_eq!(s.name, lib.sequences[0].1.name);
        assert!(preview_sequence_of(&lib, pal.clone(), SeqRef::Generated, m, 1).is_ok());
        let pat = light_core::patterns::Pattern::Halves;
        assert!(preview_sequence_of(&lib, pal.clone(), SeqRef::Pattern(pat), m, 1).is_ok());
        assert!(preview_sequence_of(&lib, pal.clone(), SeqRef::Id(999_999), m, 1).is_err());
        let mut bad = pal;
        bad.principal = Hsv { h: 361, s: 0, v: 0 };
        assert!(preview_sequence_of(&lib, bad.clone(), SeqRef::Generated, m, 1).is_err());
        assert!(preview_sequence_of(&lib, bad, SeqRef::Pattern(pat), m, 1).is_err());
    }

    #[test]
    fn closing_the_editor_switches_the_preview_off() {
        let (state, rx) = full_queue();
        let emit: Emit = Arc::new(|_| {});
        state.editor_closed(&emit);
        assert!(matches!(rx.recv(), Ok(Input::PopoverShown)));
        let got: Vec<Input> = (0..3)
            .map(|_| rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap())
            .collect();
        assert!(got.iter().any(|i| matches!(i, Input::PreviewPalette(None))));
        assert!(got.iter().any(|i| matches!(i, Input::Calibrate(false))));
    }

    #[test]
    fn status_flows_while_either_window_shows() {
        let state = AppState::default();
        let flag = state.visible_flag();
        assert!(!flag.load(Ordering::Relaxed));
        state.set_editor_visible(true);
        assert!(flag.load(Ordering::Relaxed));
        state.set_popover_visible(true);
        state.set_editor_visible(false);
        assert!(flag.load(Ordering::Relaxed), "the popover still shows");
        state.set_popover_visible(false);
        assert!(
            !flag.load(Ordering::Relaxed),
            "both hidden: nothing is sent"
        );
    }

    #[test]
    fn status_serialises_as_the_ui_expects() {
        let json = serde_json::to_string(&Status::initial()).unwrap_or_default();
        assert_eq!(
            json,
            r#"{"strip":{"kind":"notConfigured"},"bpm":null,"playing":false,"beatInBar":null,"barInPhrase":null,"phraseBars":8,"moment":"rhythm","auto":true,"predicted":false,"lightsOn":true,"midi":{"source":null,"ok":true},"audio":{"ok":false},"selection":{"set":"Auto","paletteAuto":true,"palette":{"name":"Sunset","colours":[{"h":20,"s":950,"v":1000},{"h":280,"s":900,"v":950},{"h":30,"s":300,"v":1000}]}},"clip":null}"#
        );
        let clip = ClipDto {
            moment: MomentDto::Calm,
            index: 1,
            elapsed: 3,
            total: 16,
        };
        assert_eq!(
            serde_json::to_string(&clip).unwrap_or_default(),
            r#"{"moment":"calm","index":1,"elapsed":3,"total":16}"#
        );
        let connected = StripStatus::Connected {
            name: "dj lights".into(),
            segments: 10,
            reported: 10,
            ip: None,
        };
        assert_eq!(
            serde_json::to_string(&connected).unwrap_or_default(),
            r#"{"kind":"connected","name":"dj lights","segments":10,"reported":10}"#
        );
    }

    #[test]
    fn release_manifest_has_no_dev_commands() {
        let build = include_str!("../build.rs");
        assert!(!build.contains("dev_"), "build.rs must list no dev command");
        let list = build
            .split("COMMANDS: &[&str] = &[")
            .nth(1)
            .and_then(|r| r.split("];").next())
            .unwrap_or_default();
        let mut commands: Vec<String> = list
            .split('"')
            .skip(1)
            .step_by(2)
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect();
        // The popover and editor capabilities together grant exactly those commands.
        let mut granted: Vec<String> = Vec::new();
        for cap in [
            include_str!("../capabilities/popover.json"),
            include_str!("../capabilities/editor.json"),
        ] {
            let cap: serde_json::Value = serde_json::from_str(cap).unwrap_or_default();
            granted.extend(
                cap["permissions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str())
                    .filter(|p| p.starts_with("allow-"))
                    .map(String::from),
            );
        }
        granted.sort();
        granted.dedup();
        commands.sort();
        assert_eq!(commands.len(), 29);
        assert_eq!(granted, commands);
    }

    #[test]
    fn thresholds_validated_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let state = AppState::default();
        state.init_settings(path.clone());
        let t = |kick, high, riser| Thresholds {
            calm_kick_db: kick,
            calm_high_db: high,
            riser_db: riser,
        };
        let saved = state.set_thresholds(t(20, 10, 5)).unwrap();
        assert_eq!(
            (saved.calm_kick_db, saved.calm_high_db, saved.riser_db),
            (20, 10, 5)
        );
        assert_eq!(settings::load(&path), saved, "persisted");
        for bad in [
            t(7, 12, 7),
            t(31, 12, 7),
            t(16, 5, 7),
            t(16, 25, 7),
            t(16, 12, 2),
            t(16, 12, 16),
        ] {
            assert!(state.set_thresholds(bad).is_err());
        }
        assert_eq!(state.settings(), saved, "a refused value changes nothing");
        assert!(
            serde_json::from_str::<Thresholds>(
                r#"{"calmKickDb":16.5,"calmHighDb":12,"riserDb":7}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Thresholds>(r#"{"calmKickDb":"x","calmHighDb":12,"riserDb":7}"#)
                .is_err()
        );
    }

    #[test]
    fn strip_state_is_readable_without_a_status_publish() {
        let state = AppState::default();
        let on = StripStatus::Connected {
            name: "dj".into(),
            segments: 12,
            reported: 12,
            ip: None,
        };
        state.set_strip(on.clone());
        assert_eq!(lock(&state.status).strip, on);
    }

    #[test]
    fn identify_check_needs_a_connected_strip_and_a_segment_in_range() {
        let on = StripStatus::Connected {
            name: "dj".into(),
            segments: 10,
            reported: 10,
            ip: None,
        };
        assert_eq!(identify_check(&on, 1), Ok(()));
        assert_eq!(identify_check(&on, 10), Ok(()));
        for bad in [0, 11] {
            assert_eq!(identify_check(&on, bad), Err("No such segment.".into()));
        }
        assert_eq!(
            identify_check(&StripStatus::Connecting, 1),
            Err("Strip not connected.".into())
        );
    }

    #[test]
    fn message_counts_matches_scene_diff() {
        use Role::{Contraste as C, Off, Principal as P};
        use light_core::model::{Role, Step, StepLen};
        let lib = Library::builtin(4);
        let step = |roles: [Role; 4]| Step {
            len: StepLen::One,
            roles: roles.to_vec(),
        };
        let seq = Sequence {
            name: "t".into(),
            moment: Moment::Rhythm,
            steps: vec![
                step([P, P, P, P]),         // from the last (all off): 1 bucket
                step([C, P, Off, Off]),     // a pencil and an eraser: 2
                step([Off, Off, Off, Off]), // 1 eraser
                step([Off, Off, Off, Off]), // unchanged: 0
            ],
        };
        assert_eq!(message_counts_of(&lib, &seq, 1), Ok(vec![1, 2, 1, 0]));
        assert!(message_counts_of(&lib, &seq, 99).is_err());
        let mut short = seq.clone();
        short.steps[1].roles.pop();
        assert!(message_counts_of(&lib, &short, 1).is_err());
        short.steps.clear();
        assert!(message_counts_of(&lib, &short, 1).is_err());
        let mut long = seq.clone();
        long.steps = vec![seq.steps[0].clone(); light_core::library::MAX_STEPS + 1];
        assert!(
            message_counts_of(&lib, &long, 1).is_err(),
            "IPC input is capped"
        );
    }

    #[test]
    fn preview_generated_validates_for_current_n() {
        for n in [1u8, 4, 10, 16] {
            let lib = Library::builtin(n);
            for m in [Moment::Rhythm, Moment::Calm, Moment::BuildUp] {
                let seq = preview_generated_of(&lib, m, 42 + u64::from(n)).unwrap();
                assert_eq!(seq.moment, m);
                assert!(seq.validate(n).is_ok());
            }
        }
    }

    fn lib_state() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::default();
        state.init_library(dir.path().join("library.json"), 10);
        (dir, state)
    }

    #[test]
    fn unsaved_builtin_library_follows_the_strip_but_a_saved_one_does_not() {
        let (dir, state) = lib_state();
        assert_eq!(state.rebuild_unsaved_library(10), None, "same count");
        let lib = state.rebuild_unsaved_library(12).expect("rebuilt");
        assert_eq!((lib.segments(), lib.validate()), (12, Ok(())));
        assert_eq!(state.library(), lib);
        assert_eq!(lib.sets[0].name, "Auto");
        // Once saved it is the user's.
        state.save_library(lib.clone()).unwrap();
        assert!(dir.path().join("library.json").exists());
        assert_eq!(state.rebuild_unsaved_library(6), None);
        assert_eq!(state.library(), lib);
    }

    fn state_with_strip(n: u8, m: u8) -> (tempfile::TempDir, AppState) {
        let (dir, state) = lib_state();
        *lock(&state.settings_path) = Some(dir.path().join("settings.json"));
        let cfg = strip_store::StripConfig {
            id: "abc".into(),
            name: "n".into(),
            local_key: secrecy::SecretString::from("0123456789abcdef"),
            segments: n,
            reported: m,
            last_ip: None,
        };
        strip_store::save(&dir.path().join("strip.json"), &cfg).unwrap();
        (dir, state)
    }

    #[test]
    fn connected_status_carries_the_reported_count() {
        let s = StripStatus::Connected {
            name: "dj".into(),
            segments: 8,
            reported: 10,
            ip: None,
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(
            (v["segments"].as_u64(), v["reported"].as_u64()),
            (Some(8), Some(10))
        );
    }

    #[test]
    fn connected_status_shows_only_a_private_ip() {
        let mut c = strip_store::StripConfig {
            id: "bf1234".into(),
            name: "dj".into(),
            local_key: secrecy::SecretString::from("0123456789abcdef"),
            segments: 8,
            reported: 10,
            last_ip: Some("192.168.1.42".parse().unwrap()),
        };
        let json = |c: &strip_store::StripConfig| {
            serde_json::to_value(crate::strip::connected_status(c)).unwrap()
        };
        assert_eq!(json(&c)["ip"], "192.168.1.42");
        c.last_ip = Some("8.8.8.8".parse().unwrap());
        assert!(json(&c).get("ip").is_none());
    }

    #[test]
    fn growing_continues_along_an_off_axis_piece() {
        let mut lib = Library::builtin(1);
        lib.shape.pieces[0] = light_core::library::Piece {
            x: 10.0,
            y: 10.0,
            angle: 45,
            len: 1,
        };
        let grown = with_wired(&lib, 2);
        let p = grown.shape.pieces[1];
        assert!(
            (p.x - 10.707).abs() < 0.01 && (p.y - 10.707).abs() < 0.01,
            "{p:?}"
        );
    }

    #[test]
    fn set_wired_segments_keeps_existing_pieces() {
        let (dir, state) = state_with_strip(10, 10);
        let before = state.library();
        let lib = state.set_wired(8).unwrap();
        assert_eq!(lib.shape.pieces, before.shape.pieces[..8]);
        assert_eq!(lib.sequences[0].1.steps[0].roles.len(), 8);
        let cfg = strip_store::load(&dir.path().join("strip.json")).unwrap();
        assert_eq!((cfg.segments, cfg.reported), (8, 10));
        // Growing continues the row after the last piece; the others are untouched.
        let lib = state.set_wired(10).unwrap();
        assert_eq!(lib.shape.pieces[..8], before.shape.pieces[..8]);
        assert_eq!((lib.shape.pieces[9].x, lib.shape.pieces[9].y), (9.0, 0.0));
        assert_eq!(lib.validate(), Ok(()));
        assert!(state.set_wired(0).is_err());
        assert!(state.set_wired(11).is_err());
    }

    #[test]
    fn key_file_mode_stays_0600_after_resize() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, state) = state_with_strip(10, 10);
        state.set_wired(6).unwrap();
        let mode = std::fs::metadata(dir.path().join("strip.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn save_library_rejects_invalid_and_keeps_previous() {
        let (dir, state) = lib_state();
        let before = state.library();
        let mut bad = before.clone();
        bad.sets[0].palettes = vec![4242];
        let msg = state.save_library(bad).unwrap_err();
        assert!(msg.contains("palettes") && !msg.contains("4242"), "{msg}");
        assert_eq!(state.library(), before);
        assert!(!dir.path().join("library.json").exists());
    }

    #[test]
    fn saving_a_shape_with_another_piece_count_resizes_the_roles() {
        use light_core::library::Shape;
        use light_core::model::Role;
        let (dir, state) = lib_state();
        for m in [12u8, 6] {
            let mut lib = state.library();
            lib.shape = Shape::row(m);
            let saved = state.save_library(lib).unwrap();
            assert_eq!(saved.segments(), m);
            for (_, q) in &saved.sequences {
                // The same predicate the engine uses to decide the library fits the strip.
                assert_eq!(q.validate(m), Ok(()));
            }
            if m == 12 {
                assert_eq!(saved.sequences[0].1.steps[0].roles[10..], [Role::Off; 2]);
            }
            assert_eq!(
                crate::library_store::load(&dir.path().join("library.json"), 10, 8),
                saved
            );
        }
        // A client that sends wrong-length roles for an unchanged shape is still refused.
        let mut bad = state.library();
        bad.sequences[0].1.steps[0].roles.pop();
        assert!(state.save_library(bad).is_err());
    }

    fn full_queue() -> (AppState, std::sync::mpsc::Receiver<Input>) {
        let state = AppState::default();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        tx.send(Input::PopoverShown).unwrap();
        state.set_runtime(RuntimeHandle { tx });
        (state, rx)
    }

    #[test]
    fn a_full_queue_still_gets_the_library_and_the_meters_off() {
        let (state, rx) = full_queue();
        state.deliver(Input::Library(Box::new(Library::builtin(10))));
        let emit: Emit = Arc::new(|_| {});
        state.editor_closed(&emit);
        assert!(matches!(rx.recv(), Ok(Input::PopoverShown)));
        let got: Vec<Input> = (0..4)
            .map(|_| rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap())
            .collect();
        assert!(got.iter().any(|i| matches!(i, Input::Library(_))));
        assert!(
            got.iter()
                .any(|i| matches!(i, Input::DetectionWatch(false)))
        );
    }

    #[test]
    fn select_and_save_succeed_when_the_queue_is_full() {
        let (dir, state) = lib_state();
        let (full, _rx) = full_queue();
        *lock(&state.runtime) = lock(&full.runtime).take();
        assert!(select(&state, Pick::Set, 1).is_ok());
        assert!(save_to(&state, state.library()).is_ok());
        drop(dir);
    }

    #[test]
    fn palette_cycle_includes_auto() {
        let (dir, state) = lib_state();
        let name = |sel: Selection| (sel.palette_auto, sel.palette.name);
        assert!(state.library().active_palette.is_none());
        // Auto -> p1 -> p2 ... -> p8 -> Auto, over the active set's palettes.
        assert_eq!(
            state.select(Pick::Palette, 1).unwrap().palette.name,
            "Sunset"
        );
        assert_eq!(state.library().active_palette, Some(1));
        assert_eq!(
            name(state.select(Pick::Palette, 1).unwrap()),
            (false, "Ocean".into())
        );
        assert_eq!(
            crate::library_store::load(&dir.path().join("library.json"), 10, 8).active_palette,
            Some(2)
        );
        state.select(Pick::Palette, -1).unwrap();
        assert!(state.select(Pick::Palette, -1).unwrap().palette_auto);
        assert_eq!(state.library().active_palette, None);
        // Backwards from Auto lands on the last palette, forwards from it on Auto.
        state.select(Pick::Palette, -1).unwrap();
        assert_eq!(state.library().active_palette, Some(8));
        assert!(state.select(Pick::Palette, 1).unwrap().palette_auto);
        for bad in [0, 2, -2] {
            assert!(state.select(Pick::Palette, bad).is_err());
            assert!(state.select(Pick::Set, bad).is_err());
        }
    }

    fn two_set_state() -> (tempfile::TempDir, AppState) {
        let (dir, state) = lib_state();
        let mut lib = state.library();
        let mut set = lib.sets[0].clone();
        set.name = "Club".into();
        set.id = 900;
        set.palettes = vec![3, 4];
        lib.sets.push(set);
        state.save_library(lib).unwrap();
        (dir, state)
    }

    #[test]
    fn save_keeps_live_selection() {
        let (_dir, state) = two_set_state();
        state.set_live_set(2).unwrap();
        state.select(Pick::Palette, 1).unwrap(); // pins 3
        let mut lib = state.library();
        lib.active_set = 1;
        lib.active_palette = None;
        lib.sets[1].name = "Renamed".into();
        let saved = state.save_library(lib).unwrap();
        assert_eq!((saved.active_set, saved.active_palette), (2, Some(3)));
        assert_eq!(saved.sets[1].name, "Renamed");
        // Deleting the live set falls back to the first one.
        let mut lib = saved;
        lib.sets.pop();
        let saved = state.save_library(lib).unwrap();
        assert_eq!((saved.active_set, saved.active_palette), (1, Some(3)));
    }

    #[test]
    fn save_drops_pin_not_in_active_set() {
        let (_dir, state) = two_set_state();
        state.set_live_set(2).unwrap();
        state.select(Pick::Palette, 1).unwrap();
        assert_eq!(state.library().active_palette, Some(3));
        let mut lib = state.library();
        lib.sets[1].palettes = vec![4];
        let saved = state.save_library(lib).unwrap();
        assert_eq!((saved.active_set, saved.active_palette), (2, None));
    }

    #[test]
    fn set_live_set_validates_and_clears_foreign_pin() {
        let (_dir, state) = two_set_state();
        state.select(Pick::Palette, 1).unwrap(); // pins 1 in set 1
        assert!(state.set_live_set(77).is_err());
        assert_eq!(state.library().active_set, 1);
        let sel = state.set_live_set(2).unwrap();
        assert_eq!((sel.set.as_str(), sel.palette_auto), ("Club", true));
        assert_eq!(state.library().active_palette, None);
    }

    #[test]
    fn pattern_sequence_works_without_strip() {
        use light_core::patterns::Pattern;
        // No strip connected: the library's own segment count rules.
        let lib = Library::builtin(8);
        let seq = pattern_sequence_of(&lib, Pattern::Halves, 7).unwrap();
        assert!(seq.validate(8).is_ok());
        assert!(seq.steps.iter().all(|s| s.roles.len() == 8));
        assert_eq!(seq, pattern_sequence_of(&lib, Pattern::Halves, 7).unwrap());
    }

    #[test]
    fn pattern_sequence_rejects_unknown_pattern() {
        use light_core::patterns::Pattern;
        assert!(serde_json::from_str::<Pattern>(r#""nope""#).is_err());
        assert!(serde_json::from_str::<Pattern>(r#""halves""#).is_ok());
    }

    #[test]
    fn set_cycles_through_sets() {
        let (_dir, state) = lib_state();
        let mut lib = state.library();
        let mut set = lib.sets[0].clone();
        set.name = "Club".into();
        set.id = 900;
        lib.sets.push(set);
        state.save_library(lib).unwrap();
        assert_eq!(state.select(Pick::Set, 1).unwrap().set, "Club");
        assert_eq!(state.select(Pick::Set, 1).unwrap().set, "Auto");
        assert_eq!(state.select(Pick::Set, -1).unwrap().set, "Club");
    }

    #[test]
    fn new_entries_get_ids_in_rust() {
        use light_core::library::SeqRef;
        let (dir, state) = lib_state();
        let mut lib = state.library();
        // Ids the stored library does not have are new: the client's number is only a handle.
        let pal = lib.palettes[0].1.clone();
        let seq = lib.sequences[0].1.clone();
        lib.palettes.push((900, pal));
        lib.sequences.push((901, seq));
        let mut set = lib.sets[0].clone();
        set.id = 902;
        set.rhythm.sequences = vec![SeqRef::Id(901)];
        set.palettes = vec![900];
        lib.sets.push(set);
        lib.active_set = 902;
        lib.active_palette = Some(900);

        let saved = state.save_library(lib).unwrap();
        assert_eq!(
            saved.palettes.iter().map(|p| p.0).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        assert_eq!(
            saved.sequences.iter().map(|p| p.0).collect::<Vec<_>>(),
            (1..=11).collect::<Vec<_>>()
        );
        assert_eq!(saved.sets[1].id, 2);
        assert_eq!(saved.sets[1].rhythm.sequences, [SeqRef::Id(11)]);
        assert_eq!(saved.sets[1].palettes, [9]);
        // The editor's selection is ignored: the live one stays.
        assert_eq!((saved.active_set, saved.active_palette), (1, None));
        assert_eq!(state.library(), saved);
        assert_eq!(
            crate::library_store::load(&dir.path().join("library.json"), 10, 8),
            saved
        );
    }

    #[test]
    fn ui_command_dto_parses_and_rejects_unknown() {
        let parse = |j: &str| serde_json::from_str::<UiCommandDto>(j);
        assert_eq!(
            parse(r#"{"kind":"lights","on":false}"#).ok(),
            Some(UiCommandDto::Lights { on: false })
        );
        assert_eq!(
            UiCommand::from(parse(r#"{"kind":"force","moment":"buildUp"}"#).unwrap()),
            UiCommand::Force(Moment::BuildUp)
        );
        assert_eq!(parse(r#"{"kind":"auto"}"#).ok(), Some(UiCommandDto::Auto));
        assert!(parse(r#"{"kind":"reboot"}"#).is_err());
        assert!(parse(r#"{"kind":"force","moment":"loud"}"#).is_err());
    }
}
