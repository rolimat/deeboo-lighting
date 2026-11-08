//! The one monotonic clock of the live runtime: MIDI timestamps, engine deadlines and strip
//! scheduling all count microseconds from the same start.

use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct MonoClock(Instant);

impl MonoClock {
    #[must_use]
    pub fn new() -> Self {
        Self(Instant::now())
    }

    #[must_use]
    pub fn now_us(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

impl Default for MonoClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_goes_back_and_counts_microseconds() {
        let c = MonoClock::new();
        let a = c.now_us();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = c.now_us();
        assert!(b >= a + 2_000);
    }
}
