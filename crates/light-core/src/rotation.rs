//! Rotation of a moment's sequences and of the set's palettes (spec §6.1-§6.2): which one plays next, and when.
use crate::clock::Position;
use crate::generator::SplitMix64;
use crate::library::{Interval, MomentSet, Order, PaletteChange, SeqRef, Set};

/// Next index in `0..len` after `cur`, or `None` when nothing can change. Random never repeats.
fn advance(order: Order, rng: &mut SplitMix64, len: usize, cur: usize) -> Option<usize> {
    if len < 2 {
        return None;
    }
    match order {
        Order::Fixed => None,
        Order::InOrder => Some((cur + 1) % len),
        Order::Random => {
            let r = usize::try_from(rng.below(len as u64 - 1)).unwrap_or(0);
            Some(if r >= cur { r + 1 } else { r })
        }
    }
}

/// True on the bar start `beats` or more after the last change, which it then records. Only a bar
/// start counts; a MIDI Start restarts the beat count, so a `last` in the future counts from now.
fn due(last: &mut Option<u64>, pos: Position, beats: u64) -> bool {
    if !pos.is_bar_start() {
        return false;
    }
    let l = (*last.get_or_insert(pos.beat)).min(pos.beat);
    *last = Some(l);
    let due = pos.beat.saturating_sub(l) >= beats.max(1);
    if due {
        *last = Some(pos.beat);
    }
    due
}

#[derive(Clone, Debug)]
pub struct Rotator {
    seqs: Vec<SeqRef>,
    order: Order,
    interval: Interval,
    rng: SplitMix64,
    seq_i: usize,
    /// Beat of the last change (or of the first bar start seen).
    last: Option<u64>,
    /// Calm: the first entry plays the current sequence, later ones advance.
    entered_once: bool,
}

impl Rotator {
    #[must_use]
    pub fn new(set: &MomentSet, seed: u64) -> Self {
        Self {
            seqs: set.sequences.clone(),
            order: set.order,
            interval: set.interval,
            rng: SplitMix64::new(seed),
            seq_i: 0,
            last: None,
            entered_once: false,
        }
    }

    /// The sequence in force (the set's first until the first change).
    #[must_use]
    pub fn current(&self) -> Option<SeqRef> {
        self.seqs.get(self.seq_i).copied()
    }

    /// Called at bar starts (and on entering the moment, `entered`). `Bars(n)` counts from the last
    /// change and fires only on a bar start; `OncePerBreak` advances on each entry after the first.
    pub fn on_boundary(&mut self, pos: Position, entered: bool) -> Option<SeqRef> {
        let fire = match self.interval {
            Interval::OncePerBreak => {
                let advance = entered && self.entered_once;
                self.entered_once |= entered;
                advance
            }
            Interval::Bars(n) => {
                if entered {
                    self.last = Some(pos.beat);
                }
                due(&mut self.last, pos, u64::from(n) * 4)
            }
        };
        if !fire {
            return None;
        }
        if let Some(i) = advance(self.order, &mut self.rng, self.seqs.len(), self.seq_i) {
            self.seq_i = i;
            return Some(self.seqs[i]);
        }
        // A lone `Generated` or pattern is rebuilt with new params every time.
        match self.seqs[..] {
            [r @ (SeqRef::Generated | SeqRef::Pattern(_))] if self.order != Order::Fixed => Some(r),
            _ => None,
        }
    }
}

/// The active set's palette rotation, independent of the moments.
#[derive(Clone, Debug)]
pub struct PaletteRotator {
    pals: Vec<u32>,
    order: Order,
    change: PaletteChange,
    rng: SplitMix64,
    i: usize,
    last: Option<u64>,
}

impl PaletteRotator {
    /// Starts at the palette `shown` when the set has it, so the first change moves to another one.
    #[must_use]
    pub fn new(set: &Set, seed: u64, shown: Option<u32>) -> Self {
        let i = shown
            .and_then(|id| set.palettes.iter().position(|p| *p == id))
            .unwrap_or(0);
        Self {
            pals: set.palettes.clone(),
            order: set.palette_order,
            change: set.palette_change,
            rng: SplitMix64::new(seed),
            i,
            last: None,
        }
    }

    /// The palette the rotation is at.
    #[must_use]
    pub fn current(&self) -> Option<u32> {
        self.pals.get(self.i).copied()
    }

    fn step(&mut self) -> Option<u32> {
        self.i = advance(self.order, &mut self.rng, self.pals.len(), self.i)?;
        self.current()
    }

    /// A drop: the next palette when the set changes `AtDrop`.
    pub fn on_drop(&mut self) -> Option<u32> {
        (self.change == PaletteChange::AtDrop)
            .then(|| self.step())
            .flatten()
    }

    /// Every bar start: the next palette when `Bars(n)` bars have passed since the last change.
    pub fn on_bar(&mut self, pos: Position) -> Option<u32> {
        let PaletteChange::Bars(n) = self.change else {
            return None;
        };
        if due(&mut self.last, pos, u64::from(n) * 4) {
            self.step()
        } else {
            None
        }
    }
}
