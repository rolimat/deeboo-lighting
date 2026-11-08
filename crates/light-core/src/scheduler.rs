//! Send scheduler: spaces strip messages, drops stale ones (spec §6.5).

use std::collections::VecDeque;

use crate::model::Colour;
use crate::paint::{Hsv, PaintCommand};
use crate::scene::{apply, diff};

/// Minimum gap between two messages to the strip.
pub const MESSAGE_GAP_US: u64 = 125_000;

/// Matches no real scene, so the next diff repaints everything.
const UNKNOWN: Colour = Some(Hsv {
    h: u16::MAX,
    s: u16::MAX,
    v: u16::MAX,
});

pub struct Scheduler {
    sent: Vec<Colour>,
    pending: VecDeque<(u64, PaintCommand)>,
    last_send_us: Option<u64>,
}

impl Scheduler {
    #[must_use]
    pub fn new(segments: u8) -> Self {
        Self {
            sent: vec![None; usize::from(segments)],
            pending: VecDeque::new(),
            last_send_us: None,
        }
    }

    /// Replaces unsent messages with the diff of `desired` against the sent state.
    pub fn plan(&mut self, now_us: u64, step_at_us: u64, lead_us: u64, desired: &[Colour]) {
        self.pending.clear();
        let mut at = step_at_us.saturating_sub(lead_us).max(now_us);
        if let Some(last) = self.last_send_us {
            at = at.max(last.saturating_add(MESSAGE_GAP_US));
        }
        for cmd in diff(&self.sent, desired) {
            self.pending.push_back((at, cmd));
            at = at.saturating_add(MESSAGE_GAP_US);
        }
    }

    /// Messages whose time has come, in order; they join the sent state.
    /// At most one message per call: if the caller wakes up late, the rest are pushed back so the
    /// strip always gets at least `MESSAGE_GAP_US` between real sends.
    pub fn due(&mut self, now_us: u64) -> Vec<PaintCommand> {
        match self.pending.front() {
            Some((at, _)) if *at <= now_us => {}
            _ => return Vec::new(),
        }
        let Some((_, cmd)) = self.pending.pop_front() else {
            return Vec::new();
        };
        apply(&mut self.sent, &cmd);
        self.last_send_us = Some(now_us);
        let mut earliest = now_us.saturating_add(MESSAGE_GAP_US);
        for (at, _) in &mut self.pending {
            *at = (*at).max(earliest);
            earliest = at.saturating_add(MESSAGE_GAP_US);
        }
        vec![cmd]
    }

    #[must_use]
    pub fn next_deadline_us(&self) -> Option<u64> {
        self.pending.front().map(|(at, _)| *at)
    }

    #[must_use]
    pub fn sent(&self) -> &[Colour] {
        &self.sent
    }

    /// State unknown after a reconnect: the next plan sends a full scene.
    pub fn forget_sent(&mut self) {
        self.sent.fill(UNKNOWN);
        self.pending.clear();
    }
}
