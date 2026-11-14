//! Colour roles, palettes and step sequences.
use crate::detect::Moment;
use crate::paint::Hsv;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Role {
    Principal,
    Contraste,
    Suave,
    White,
    Off,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Palette {
    pub name: String,
    pub principal: Hsv,
    pub contraste: Hsv,
    pub suave: Hsv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum StepLen {
    Half,
    One,
    Two,
    Four,
}

impl StepLen {
    #[must_use]
    pub fn half_beats(self) -> u8 {
        match self {
            StepLen::Half => 1,
            StepLen::One => 2,
            StepLen::Two => 4,
            StepLen::Four => 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Step {
    pub len: StepLen,
    pub roles: Vec<Role>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    Empty,
    WrongSegmentCount {
        step: usize,
        expected: u8,
        found: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Sequence {
    pub name: String,
    pub moment: Moment,
    pub steps: Vec<Step>,
    /// Calm only: cross-fade between its steps.
    #[cfg_attr(feature = "serde", serde(default = "fade_default"))]
    pub fade: bool,
}

#[cfg(feature = "serde")]
fn fade_default() -> bool {
    true
}

impl Sequence {
    /// # Errors
    /// `Empty` without steps; `WrongSegmentCount` for the first step whose role count differs.
    pub fn validate(&self, segments: u8) -> Result<(), ModelError> {
        if self.steps.is_empty() {
            return Err(ModelError::Empty);
        }
        match self
            .steps
            .iter()
            .position(|s| s.roles.len() != usize::from(segments))
        {
            Some(step) => Err(ModelError::WrongSegmentCount {
                step,
                expected: segments,
                found: self.steps[step].roles.len(),
            }),
            None => Ok(()),
        }
    }
}

/// `None` = off.
pub type Colour = Option<Hsv>;

#[must_use]
pub fn resolve(role: Role, palette: &Palette, brightness_pct: u8) -> Colour {
    let base = match role {
        Role::Principal => palette.principal,
        Role::Contraste => palette.contraste,
        Role::Suave => palette.suave,
        Role::White => Hsv {
            h: 0,
            s: 0,
            v: 1000,
        },
        Role::Off => return None,
    };
    let pct = u32::from(brightness_pct.clamp(10, 100));
    let v = (u32::from(base.v) * pct + 50) / 100;
    Some(Hsv {
        v: u16::try_from(v).unwrap_or(1000),
        ..base
    })
}
