//! Show engine: clock events to strip messages, in simulated time (spec §6.1-§6.6).
use crate::builtin;
use crate::clock::{ClockEvent, ClockTracker, PhraseBars, Position};
use crate::detect::Moment;
use crate::generator::Generator;
use crate::groups::{Groups, groups};
use crate::library::{Library, MomentSet, SeqRef, Set};
use crate::model::{Colour, Palette, Role, Sequence, resolve};
use crate::moments::MomentControl;
use crate::paint::PaintCommand;
use crate::rotation::{Choice, Rotator};
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
    /// Only for a library built for this strip: rotation state and what `Generated` needs.
    rot: Option<Box<Rotation>>,
}

#[derive(Clone)]
struct Rotation {
    lib: Library,
    groups: Groups,
    rots: [Rotator; 3],
}

const MOMENTS: [Moment; 3] = [Moment::Rhythm, Moment::Calm, Moment::BuildUp];

fn slot(m: Moment) -> usize {
    MOMENTS.iter().position(|x| *x == m).unwrap_or(0)
}

fn moment_set(set: &Set, m: Moment) -> &MomentSet {
    match m {
        Moment::Rhythm => &set.rhythm,
        Moment::Calm => &set.calm,
        Moment::BuildUp => &set.buildup,
    }
}

/// A library sequence, or a generated one; the built-in when the id is gone or invalid.
fn resolve_seq(r: &Rotation, generator: &mut Generator, m: Moment, sr: SeqRef, n: u8) -> Sequence {
    match sr {
        SeqRef::Id(id) => r
            .lib
            .sequences
            .iter()
            .find(|(i, q)| *i == id && q.validate(n).is_ok())
            .map(|(_, q)| q.clone()),
        SeqRef::Generated => Some(generator.next(m, &r.groups, n)),
    }
    .unwrap_or_else(|| builtin::sequence(m, n))
}

impl Look {
    fn builtin(segments: u8) -> Self {
        Self {
            palette: builtin::test_palette(),
            seqs: MOMENTS.map(|m| builtin::sequence(m, segments)),
            rot: None,
        }
    }

    /// Per moment the active set's first sequence (generated ones built now, once), with rotators
    /// for the rest. A library built for another segment count, or without an active set, plays the
    /// built-ins. The popover's palette pins the palette until the next drop.
    fn from_library(lib: &Library, segments: u8, seed: u64, generator: &mut Generator) -> Self {
        let Some(set) = lib.current_set().filter(|_| lib.segments() == segments) else {
            return Self {
                palette: lib.current_palette(),
                ..Self::builtin(segments)
            };
        };
        let rots = MOMENTS.map(|m| {
            let r = Rotator::new(moment_set(set, m), seed ^ (slot(m) as u64 + 1) << 56)
                .with_palette(lib.current_palette_id());
            if m == Moment::Calm { r.per_entry() } else { r }
        });
        let rot = Rotation {
            lib: lib.clone(),
            groups: groups(&lib.shape),
            rots,
        };
        let seqs = MOMENTS.map(|m| {
            rot.rots[slot(m)].current().map_or_else(
                || builtin::sequence(m, segments),
                |sr| resolve_seq(&rot, generator, m, sr, segments),
            )
        });
        Self {
            palette: lib.current_palette(),
            seqs,
            rot: Some(Box::new(rot)),
        }
    }
}

impl Look {
    /// The same look for a new delivery of a library that changed nothing the rotation depends on
    /// (shape, sequences, active set): rotation state and playing sequences stay, only the library
    /// data and a changed pinned palette are taken. `None` when it has to be rebuilt.
    fn kept(&self, lib: &Library, segments: u8) -> Option<Self> {
        let old = &self.rot.as_ref()?.lib;
        let same = lib.segments() == segments
            && old.current_set() == lib.current_set()
            && old.sequences == lib.sequences
            && old.shape == lib.shape;
        if !same {
            return None;
        }
        let mut look = self.clone();
        let r = look.rot.as_mut()?;
        if old.active_palette != lib.active_palette {
            look.palette = lib.current_palette();
            for rot in &mut r.rots {
                rot.sync_palette(lib.current_palette_id());
            }
        }
        r.lib = lib.clone();
        Some(look)
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
    generator: Generator,
    seed: u64,
    /// Library rebuilds so far: mixed into the seed so a rebuilt rotation does not replay the last.
    rebuilds: u64,
    /// The moment the last step was planned for, to see entries and drops.
    rot_moment: Option<Moment>,
    playing: Option<Playing>,
    pos: Option<Position>,
    last_beat_us: Option<u64>,
    /// The strip may not show the current step: plan it again on the next pulse.
    resend: bool,
}

impl Engine {
    #[must_use]
    pub fn new(cfg: EngineConfig) -> Self {
        Self::with_seed(cfg, 0)
    }

    /// `seed` drives random order and generated sequences (the app passes the launch time).
    #[must_use]
    pub fn with_seed(cfg: EngineConfig, seed: u64) -> Self {
        Self {
            generator: Generator::new(seed),
            seed,
            rebuilds: 0,
            rot_moment: None,
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
        let segments = self.cfg.segments;
        let kept = self
            .pending
            .as_ref()
            .unwrap_or(&self.look)
            .kept(lib, segments);
        let look = kept.unwrap_or_else(|| {
            self.rebuilds += 1;
            let seed = self
                .seed
                .wrapping_add(self.rebuilds.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            Look::from_library(lib, segments, seed, &mut self.generator)
        });
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
        if start {
            // A Start (also while running) begins a fresh show: rotation counts from here.
            self.rot_moment = None;
        }
        let entered = self.rot_moment != Some(moment);
        let dropped =
            entered && self.rot_moment == Some(Moment::BuildUp) && moment == Moment::Rhythm;
        self.rot_moment = Some(moment);
        let bar = start || (pos.half && pos.beat_in_bar == 4);
        if entered || bar {
            // The step planned on that half pulse is the next bar's first: count it as the bar start.
            let at_pos = if bar && pos.half {
                self.clock.position_after(pos)
            } else {
                pos
            };
            self.rotate(moment, at_pos, entered, dropped);
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

    /// Asks the moment's rotator (and, on a drop, the rhythm set's palette list) what changes now.
    /// Called only on a bar start, or when the moment is entered, where its sequence restarts anyway.
    /// A new sequence or palette restarts the step count so the strip shows it at once.
    fn rotate(&mut self, moment: Moment, pos: Position, entered: bool, dropped: bool) {
        let n = self.cfg.segments;
        let i = slot(moment);
        let Some(r) = self.look.rot.as_mut() else {
            return;
        };
        let drop_pal = dropped
            .then(|| r.rots[slot(Moment::Rhythm)].on_drop())
            .flatten()
            .map(Choice::Palette);
        let picks = [drop_pal, r.rots[i].on_boundary(pos, entered)];
        for c in picks.into_iter().flatten() {
            match c {
                Choice::Sequence(sr) => {
                    self.look.seqs[i] = resolve_seq(r, &mut self.generator, moment, sr, n);
                }
                Choice::Palette(id) => {
                    if let Some((_, p)) = r.lib.palettes.iter().find(|(x, _)| *x == id) {
                        self.look.palette = p.clone();
                    }
                }
            }
            self.playing = None;
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
        self.rot_moment = None;
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
