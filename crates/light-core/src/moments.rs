//! Moment control (spec 6.2): when a detected moment takes effect, drop prediction, manual override.
use crate::clock::{PhraseBars, Position};
use crate::detect::Moment;

#[derive(Clone, Copy, Debug)]
pub struct MomentControl {
    applied: Moment,
    forced: Option<Moment>,
    predicted: bool,
    /// Detector calls left before an unconfirmed prediction reverts.
    wait: u8,
}

impl Default for MomentControl {
    fn default() -> Self {
        Self::new()
    }
}

impl MomentControl {
    #[must_use]
    pub fn new() -> Self {
        Self {
            applied: Moment::Rhythm,
            forced: None,
            predicted: false,
            wait: 0,
        }
    }

    /// Called once per beat. `detected` describes the beat that just ended; `upcoming` is the beat
    /// being planned. Returns the moment shown from `upcoming`.
    pub fn next(
        &mut self,
        upcoming: Position,
        phrase: PhraseBars,
        detected: Option<Moment>,
    ) -> Moment {
        if let Some(m) = self.forced {
            self.applied = m;
            self.predicted = false;
            return m;
        }
        let was_buildup = self.applied == Moment::BuildUp;
        // `detected` lags: it describes the previous beat and a drop needs `rhythm_after_full` (2)
        // full beats, so a prediction waits 3 calls before the detector may contradict it.
        let was_predicted = std::mem::take(&mut self.predicted);
        let holding = was_predicted && self.wait > 1 && detected == Some(Moment::BuildUp);
        self.wait = if holding { self.wait - 1 } else { 0 };
        let phrase_start =
            upcoming.beat.is_multiple_of(4 * u64::from(phrase.bars())) && !upcoming.half;
        self.applied = match detected {
            None => Moment::Rhythm,
            Some(Moment::BuildUp) if holding => {
                self.predicted = true;
                Moment::Rhythm
            }
            // Still building after the detector's lag: correct back to BuildUp.
            Some(Moment::BuildUp) if was_predicted => Moment::BuildUp,
            Some(Moment::BuildUp) if was_buildup && phrase_start => {
                self.predicted = true;
                self.wait = 3;
                Moment::Rhythm
            }
            Some(Moment::BuildUp) if was_buildup || upcoming.is_bar_start() => Moment::BuildUp,
            // Waiting for the bar start: keep what is shown.
            Some(Moment::BuildUp) => self.applied,
            Some(m) => m,
        };
        self.applied
    }

    pub fn force(&mut self, m: Moment) {
        self.forced = Some(m);
        self.applied = m;
        self.predicted = false;
        self.wait = 0;
    }

    pub fn set_auto(&mut self) {
        self.forced = None;
    }

    #[must_use]
    pub fn auto(&self) -> bool {
        self.forced.is_none()
    }

    #[must_use]
    pub fn applied(&self) -> Moment {
        self.applied
    }

    #[must_use]
    pub fn predicted(&self) -> bool {
        self.predicted
    }

    /// Back to Rhythm; keeps auto/forced.
    pub fn reset(&mut self) {
        self.applied = Moment::Rhythm;
        self.predicted = false;
        self.wait = 0;
    }
}
