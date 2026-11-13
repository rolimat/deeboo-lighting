//! MIDI clock tracker: 24 PPQN ticks to beat, bar and phrase. Never reads a clock.
#![allow(
    clippy::cast_possible_truncation, // remainders < 16; BPM and periods are small
    clippy::cast_precision_loss,      // microsecond sums stay far below 2^52
    clippy::cast_sign_loss            // mean of positive intervals
)]

pub const PPQN: u64 = 24;
pub const STOP_TIMEOUT_US: u64 = 500_000;
const RING: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhraseBars {
    Four,
    Eight,
    Sixteen,
}

impl PhraseBars {
    #[must_use]
    pub fn bars(self) -> u8 {
        match self {
            Self::Four => 4,
            Self::Eight => 8,
            Self::Sixteen => 16,
        }
    }

    #[must_use]
    pub fn from_bars(bars: u8) -> Option<Self> {
        match bars {
            4 => Some(Self::Four),
            8 => Some(Self::Eight),
            16 => Some(Self::Sixteen),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// 0 = beat 1.
    pub beat: u64,
    /// Off-beat half.
    pub half: bool,
    /// 1..=4.
    pub beat_in_bar: u8,
    /// 1..=`phrase_bars`.
    pub bar_in_phrase: u8,
    /// Phrase length in force when this position was made.
    pub phrase_bars: u8,
}

impl Position {
    #[must_use]
    pub fn at(beat: u64, half: bool, phrase: PhraseBars) -> Self {
        let bars = phrase.bars();
        // Both remainders are below 16, so the casts cannot truncate.
        Self {
            beat,
            half,
            beat_in_bar: (beat % 4) as u8 + 1,
            bar_in_phrase: (beat / 4 % u64::from(bars)) as u8 + 1,
            phrase_bars: bars,
        }
    }

    /// The whole beat before this position's beat; `None` on the first beat.
    #[must_use]
    pub fn previous_beat(&self) -> Option<Position> {
        let phrase = PhraseBars::from_bars(self.phrase_bars)?;
        Some(Position::at(self.beat.checked_sub(1)?, false, phrase))
    }

    #[must_use]
    pub fn is_bar_start(&self) -> bool {
        self.beat_in_bar == 1 && !self.half
    }

    #[must_use]
    pub fn is_phrase_start(&self) -> bool {
        self.is_bar_start() && self.bar_in_phrase == 1
    }

    #[must_use]
    pub fn is_last_beat_of_phrase(&self) -> bool {
        self.beat_in_bar == 4 && self.bar_in_phrase == self.phrase_bars
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClockEvent {
    /// Every 12 ticks.
    Pulse {
        at_us: u64,
        pos: Position,
    },
    Stopped {
        at_us: u64,
    },
}

pub struct ClockTracker {
    phrase: PhraseBars,
    running: bool,
    armed: bool,
    tick: u64,
    last_us: u64,
    ring: [u64; RING],
    ring_len: usize,
    ring_next: usize,
}

impl ClockTracker {
    #[must_use]
    pub fn new(phrase: PhraseBars) -> Self {
        Self {
            phrase,
            running: false,
            armed: false,
            tick: 0,
            last_us: 0,
            ring: [0; RING],
            ring_len: 0,
            ring_next: 0,
        }
    }

    pub fn on_midi(&mut self, now_us: u64, status: u8) -> Option<ClockEvent> {
        match status {
            0xFA | 0xFB => {
                self.armed = true;
                None
            }
            0xFC => self.stop(now_us),
            0xF8 => self.tick_event(now_us),
            _ => None,
        }
    }

    pub fn on_deadline(&mut self, now_us: u64) -> Option<ClockEvent> {
        if self.running && now_us >= self.last_us.saturating_add(STOP_TIMEOUT_US) {
            return self.stop(now_us);
        }
        None
    }

    #[must_use]
    pub fn next_deadline_us(&self) -> Option<u64> {
        self.running
            .then(|| self.last_us.saturating_add(STOP_TIMEOUT_US))
    }

    #[must_use]
    pub fn bpm(&self) -> Option<f32> {
        let mean = self.mean_interval_us()?;
        Some((60_000_000.0 / (mean * PPQN as f64)) as f32)
    }

    #[must_use]
    pub fn half_period_us(&self) -> Option<u64> {
        Some((self.mean_interval_us()? * 12.0).round() as u64)
    }

    #[must_use]
    pub fn running(&self) -> bool {
        self.running
    }

    pub fn set_phrase(&mut self, phrase: PhraseBars) {
        self.phrase = phrase;
    }

    #[must_use]
    pub fn position_after(&self, pos: Position) -> Position {
        if pos.half {
            Position::at(pos.beat + 1, false, self.phrase)
        } else {
            Position::at(pos.beat, true, self.phrase)
        }
    }

    fn mean_interval_us(&self) -> Option<f64> {
        // Needs at least 2 pulses, i.e. 1 interval.
        if self.ring_len == 0 {
            return None;
        }
        let sum: u64 = self.ring[..self.ring_len].iter().sum();
        Some(sum as f64 / self.ring_len as f64)
    }

    fn stop(&mut self, now_us: u64) -> Option<ClockEvent> {
        self.armed = false;
        self.ring_len = 0;
        self.ring_next = 0;
        if !self.running {
            return None;
        }
        self.running = false;
        Some(ClockEvent::Stopped { at_us: now_us })
    }

    fn tick_event(&mut self, now_us: u64) -> Option<ClockEvent> {
        if self.running && now_us > self.last_us {
            self.ring[self.ring_next] = now_us - self.last_us;
            self.ring_next = (self.ring_next + 1) % RING;
            self.ring_len = (self.ring_len + 1).min(RING);
        }
        if !self.running || self.armed {
            self.tick = 0;
            self.armed = false;
            self.running = true;
        } else if now_us >= self.last_us {
            self.tick += 1; // equal timestamps (same microsecond) still count
        } else {
            return None; // time went back: not a new tick
        }
        self.last_us = now_us;
        (self.tick.is_multiple_of(12)).then(|| ClockEvent::Pulse {
            at_us: now_us,
            pos: Position::at(self.tick / PPQN, self.tick % PPQN == 12, self.phrase),
        })
    }
}
