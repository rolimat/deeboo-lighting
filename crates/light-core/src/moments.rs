//! Moment control (spec 6.2): when a detected moment takes effect, drop prediction, manual override.
use crate::clock::{PhraseBars, Position};
use crate::detect::Moment;

#[derive(Clone, Copy, Debug)]
pub struct MomentControl {
    applied: Moment,
    forced: Option<Moment>,
    predicted: bool,
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
        let was_predicted = std::mem::take(&mut self.predicted);
        let phrase_start =
            upcoming.beat.is_multiple_of(4 * u64::from(phrase.bars())) && !upcoming.half;
        self.applied = match detected {
            None => Moment::Rhythm,
            // Still building where the drop was predicted: correct back to BuildUp.
            Some(Moment::BuildUp) if was_predicted => Moment::BuildUp,
            Some(Moment::BuildUp) if was_buildup && phrase_start => {
                self.predicted = true;
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
    }
}
