//! Show engine: clock events to strip messages, in simulated time (spec §6.1-§6.6).
use crate::builtin;
use crate::clock::{ClockEvent, ClockTracker, PhraseBars, Position};
use crate::detect::Moment;
use crate::library::{Library, SeqRef};
use crate::model::{Colour, Palette, Role, Sequence, resolve};
use crate::moments::MomentControl;
use crate::paint::PaintCommand;
use crate::scheduler::Scheduler;

#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub segments: u8,
    pub phrase: PhraseBars,
    pub lead_us: u64,
    pub brightness_pct: u8,
    pub lights_on: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiCommand {
    Lights(bool),
    Force(Moment),
    Auto,
    Brightness(u8),
    Phrase(PhraseBars),
    LeadMs(u16),
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // status snapshot for the UI
pub struct EngineStatus {
    pub playing: bool,
    pub bpm: Option<f32>,
    pub pos: Option<Position>,
    pub phrase: PhraseBars,
    pub moment: Moment,
    pub auto: bool,
    pub predicted: bool,
    pub lights_on: bool,
    pub last_beat_us: Option<u64>,
}

#[derive(Clone, Copy)]
struct Playing {
    moment: Moment,
    idx: usize,
    left: u8,
}

/// What the engine plays: one palette and one sequence per moment (rhythm, calm, build-up).
#[derive(Clone)]
struct Look {
    palette: Palette,
    seqs: [Sequence; 3],
}

const MOMENTS: [Moment; 3] = [Moment::Rhythm, Moment::Calm, Moment::BuildUp];

fn slot(m: Moment) -> usize {
    MOMENTS.iter().position(|x| *x == m).unwrap_or(0)
}

impl Look {
    fn builtin(segments: u8) -> Self {
        Self {
            palette: builtin::test_palette(),
            seqs: MOMENTS.map(|m| builtin::sequence(m, segments)),
        }
    }

    /// Per moment the active set's first usable sequence, else the built-in one. A library built
    /// for another segment count plays the built-ins.
    // shortcut: rotation, Generated and per-drop palettes are plan 6.
    fn from_library(lib: &Library, segments: u8) -> Self {
        let fits = lib.segments() == segments;
        let set = lib.current_set();
        let seqs = MOMENTS.map(|m| {
            let ms = set.map(|s| match m {
                Moment::Rhythm => &s.rhythm,
                Moment::Calm => &s.calm,
                Moment::BuildUp => &s.buildup,
            });
            ms.filter(|_| fits)
                .and_then(|ms| {
                    ms.sequences.iter().find_map(|r| match r {
                        SeqRef::Id(id) => lib
                            .sequences
                            .iter()
                            .find(|(i, q)| i == id && q.validate(segments).is_ok())
                            .map(|(_, q)| q.clone()),
                        SeqRef::Generated => None,
                    })
                })
                .unwrap_or_else(|| builtin::sequence(m, segments))
        });
        Self {
            palette: lib.current_palette(),
            seqs,
        }
    }
}

pub struct Engine {
    cfg: EngineConfig,
    clock: ClockTracker,
    sched: Scheduler,
    control: MomentControl,
    look: Look,
    /// A saved library waits for the next bar start so no step is cut and nothing bursts.
    pending: Option<Look>,
    playing: Option<Playing>,
    pos: Option<Position>,
    last_beat_us: Option<u64>,
    /// The strip may not show the current step: plan it again on the next pulse.
    resend: bool,
}

impl Engine {
    #[must_use]
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            cfg,
            clock: ClockTracker::new(cfg.phrase),
            sched: Scheduler::new(cfg.segments),
            control: MomentControl::new(),
            look: Look::builtin(cfg.segments),
            pending: None,
            playing: None,
            pos: None,
            last_beat_us: None,
            resend: false,
        }
    }

    /// `detect` runs once per beat pulse (never on half pulses or other bytes).
    pub fn on_midi(
        &mut self,
        now_us: u64,
        status: u8,
        detect: impl FnOnce() -> Option<Moment>,
    ) -> Vec<PaintCommand> {
        match self.clock.on_midi(now_us, status) {
            Some(ClockEvent::Pulse { at_us, pos }) => self.pulse(at_us, pos, detect),
            Some(ClockEvent::Stopped { .. }) => self.stopped(now_us),
            None => {}
        }
        self.sched.due(now_us)
    }

    pub fn on_deadline(&mut self, now_us: u64) -> Vec<PaintCommand> {
        if let Some(ClockEvent::Stopped { .. }) = self.clock.on_deadline(now_us) {
            self.stopped(now_us);
        }
        self.sched.due(now_us)
    }

    pub fn command(&mut self, now_us: u64, cmd: UiCommand) -> Vec<PaintCommand> {
        match cmd {
            UiCommand::Lights(false) => {
                self.cfg.lights_on = false;
                self.plan_now(now_us, &vec![None; usize::from(self.cfg.segments)]);
            }
            UiCommand::Lights(true) => {
                self.cfg.lights_on = true;
                self.resend = true;
                if !self.clock.running() {
                    self.plan_rest(now_us);
                }
            }
            UiCommand::Force(m) => self.control.force(m),
            UiCommand::Auto => self.control.set_auto(),
            UiCommand::Brightness(p) => self.cfg.brightness_pct = p,
            UiCommand::Phrase(p) => {
                self.cfg.phrase = p;
                self.clock.set_phrase(p);
            }
            UiCommand::LeadMs(ms) => self.cfg.lead_us = u64::from(ms.min(500)) * 1000,
        }
        self.sched.due(now_us)
    }

    #[must_use]
    pub fn next_deadline_us(&self) -> Option<u64> {
        match (self.clock.next_deadline_us(), self.sched.next_deadline_us()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    #[must_use]
    pub fn status(&self) -> EngineStatus {
        EngineStatus {
            playing: self.clock.running(),
            bpm: self.clock.bpm(),
            pos: self.pos,
            phrase: self.cfg.phrase,
            moment: self.control.applied(),
            auto: self.control.auto(),
            predicted: self.control.predicted(),
            lights_on: self.cfg.lights_on,
            last_beat_us: self.last_beat_us,
        }
    }

    /// A stopped show uses the library at once; a running one from its next bar start.
    pub fn set_library(&mut self, now_us: u64, lib: &Library) -> Vec<PaintCommand> {
        let look = Look::from_library(lib, self.cfg.segments);
        if self.clock.running() {
            self.pending = Some(look);
        } else {
            self.look = look;
            self.pending = None;
            if self.cfg.lights_on {
                self.plan_rest(now_us);
            }
        }
        self.sched.due(now_us)
    }

    pub fn strip_reconnected(&mut self) {
        self.sched.forget_sent();
        self.resend = true;
    }

    /// Plans the scene for the position after `pos`, shown half a pulse-pair later. A moment change
    /// decided now reaches the strip from the next beat: the beat pulse plans its own second half
    /// with the moment already shown, the half pulse plans the next beat with the new one.
    fn pulse(&mut self, at: u64, pos: Position, detect: impl FnOnce() -> Option<Moment>) {
        self.pos = Some(pos);
        let shown = self.control.applied();
        if !pos.half {
            self.last_beat_us = Some(at);
            let next_beat = self.clock.position_after(self.clock.position_after(pos));
            self.control.next(next_beat, self.cfg.phrase, detect());
        }
        let moment = if pos.half {
            self.control.applied()
        } else {
            shown
        };
        let start = pos.beat == 0 && !pos.half;
        // The step planned on a half pulse of beat 4 is the first of the next bar.
        if let Some(look) = self
            .pending
            .take_if(|_| start || (pos.half && pos.beat_in_bar == 4))
        {
            self.look = look;
            self.playing = None;
        }
        // Beat 1 cannot be announced: its step goes out now and the next position is counted too.
        // shortcut: a half-beat first step is skipped on Start; only a forced Build-up has one.
        let first = start.then(|| self.step_scene(moment, true));
        let (desired, changed) = self.step_scene(moment, false);
        let ((desired, changed), show_at) = match first {
            Some(d0) => (d0, at),
            None => (
                (desired, changed),
                at.saturating_add(self.clock.half_period_us().unwrap_or(0)),
            ),
        };
        // A step that goes on keeps its pending messages: re-planning would push them back.
        if !self.cfg.lights_on {
            return;
        }
        let resend = std::mem::take(&mut self.resend);
        if let Some(desired) = desired.filter(|_| changed || resend) {
            self.sched.plan(at, show_at, self.cfg.lead_us, &desired);
        }
    }

    /// Counts one half beat of the sequence for `moment`: that step's scene, and whether it is new.
    fn step_scene(&mut self, moment: Moment, restart: bool) -> (Option<Vec<Colour>>, bool) {
        let sequence = &self.look.seqs[slot(moment)];
        let first_len = |idx: usize| sequence.steps.get(idx).map_or(1, |s| s.len.half_beats());
        let (idx, left) = match self.playing {
            Some(p) if !restart && p.moment == moment => {
                if p.left > 1 {
                    (p.idx, p.left - 1)
                } else {
                    let next = (p.idx + 1) % sequence.steps.len().max(1);
                    (next, first_len(next))
                }
            }
            _ => (0, first_len(0)),
        };
        let changed = self
            .playing
            .is_none_or(|p| p.moment != moment || p.idx != idx || restart);
        self.playing = Some(Playing { moment, idx, left });
        let scene = sequence
            .steps
            .get(idx)
            .map(|step| self.scene(step.roles.iter().copied()));
        (scene, changed)
    }

    fn stopped(&mut self, now: u64) {
        self.playing = None;
        self.pos = None;
        self.control.reset();
        if self.cfg.lights_on {
            self.plan_rest(now);
        }
    }

    fn plan_rest(&mut self, now: u64) {
        let desired = self.scene((0..self.cfg.segments).map(|_| Role::Suave));
        self.plan_now(now, &desired);
    }

    fn plan_now(&mut self, now: u64, desired: &[Colour]) {
        self.sched.plan(now, now, self.cfg.lead_us, desired);
    }

    fn scene(&self, roles: impl Iterator<Item = Role>) -> Vec<Colour> {
        roles
            .map(|r| resolve(r, &self.look.palette, self.cfg.brightness_pct))
            .collect()
    }
}
