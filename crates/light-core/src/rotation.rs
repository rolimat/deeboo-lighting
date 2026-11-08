//! Rotation of a moment's sequences and palettes (spec §6.3): which one plays next, and when.
use crate::clock::Position;
use crate::generator::SplitMix64;
use crate::library::{Interval, MomentSet, Order, SeqRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Sequence(SeqRef),
    Palette(u32),
}

#[derive(Clone, Debug)]
pub struct Rotator {
    seqs: Vec<SeqRef>,
    pals: Vec<u32>,
    order: Order,
    interval: Interval,
    rng: SplitMix64,
    seq_i: usize,
    pal_i: usize,
    /// Beat of the last change (or of the first bar start seen).
    last: Option<u64>,
    per_entry: bool,
}

impl Rotator {
    #[must_use]
    pub fn new(set: &MomentSet, seed: u64) -> Self {
        Self {
            seqs: set.sequences.clone(),
            pals: set.palettes.clone(),
            order: set.order,
            interval: set.interval,
            rng: SplitMix64::new(seed),
            seq_i: 0,
            pal_i: 0,
            last: None,
            per_entry: false,
        }
    }

    /// Calm: advance once each time the moment is entered, ignoring the interval.
    #[must_use]
    pub fn per_entry(mut self) -> Self {
        self.per_entry = true;
        self
    }

    /// The sequence in force (the set's first until the first change).
    #[must_use]
    pub fn current(&self) -> Option<SeqRef> {
        self.seqs.get(self.seq_i).copied()
    }

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

    /// Called at bar starts (and on entering the moment, `entered`). An interval counts from the
    /// last change; it fires only on a bar start, or a phrase start for phrase intervals. A set with
    /// one sequence rotates its palette instead, so the interval still shows.
    pub fn on_boundary(&mut self, pos: Position, entered: bool) -> Option<Choice> {
        let fire = if self.per_entry {
            entered
        } else {
            if entered {
                self.last = Some(pos.beat);
            }
            self.due(pos)
        };
        if !fire {
            return None;
        }
        if let Some(i) = Self::advance(self.order, &mut self.rng, self.seqs.len(), self.seq_i) {
            self.seq_i = i;
            return Some(Choice::Sequence(self.seqs[i]));
        }
        // A lone `Generated` is a new sequence every time.
        if self.order != Order::Fixed && self.seqs == [SeqRef::Generated] {
            return Some(Choice::Sequence(SeqRef::Generated));
        }
        self.on_drop().map(Choice::Palette)
    }

    fn due(&mut self, pos: Position) -> bool {
        let (beats, ok) = match self.interval {
            Interval::Beats(n) => (u64::from(n), pos.is_bar_start()),
            Interval::Bars(n) => (u64::from(n) * 4, pos.is_bar_start()),
            Interval::Phrases(n) => (
                u64::from(n) * u64::from(pos.phrase_bars) * 4,
                pos.is_phrase_start(),
            ),
        };
        if !ok {
            return false;
        }
        let last = *self.last.get_or_insert(pos.beat);
        let due = pos.beat.saturating_sub(last) >= beats.max(1);
        if due {
            self.last = Some(pos.beat);
        }
        due
    }

    /// The next palette, on a drop. `None` when the set has fewer than two or is fixed.
    pub fn on_drop(&mut self) -> Option<u32> {
        let i = Self::advance(self.order, &mut self.rng, self.pals.len(), self.pal_i)?;
        self.pal_i = i;
        Some(self.pals[i])
    }
}
