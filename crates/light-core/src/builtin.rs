//! Built-in palette and sequences (spec §6.3, §6.4).
use crate::detect::Moment;
use crate::model::{Palette, Role, Sequence, Step, StepLen};
use crate::paint::Hsv;

#[must_use]
pub fn test_palette() -> Palette {
    Palette {
        name: "Test".into(),
        principal: Hsv {
            h: 0,
            s: 1000,
            v: 1000,
        },
        contraste: Hsv {
            h: 240,
            s: 1000,
            v: 1000,
        },
        suave: Hsv {
            h: 120,
            s: 350,
            v: 1000,
        },
    }
}

fn step(len: StepLen, roles: impl Iterator<Item = Role>) -> Step {
    Step {
        len,
        roles: roles.collect(),
    }
}

#[must_use]
pub fn sequence(moment: Moment, segments: u8) -> Sequence {
    let n = usize::from(segments);
    let half = n / 2;
    let (name, steps) = match moment {
        Moment::Rhythm => (
            "Halves",
            vec![
                step(
                    StepLen::One,
                    (0..n).map(|i| if i < half { Role::Principal } else { Role::Off }),
                ),
                step(
                    StepLen::One,
                    (0..n).map(|i| if i < half { Role::Off } else { Role::White }),
                ),
            ],
        ),
        // Segments are 1-based: index 0 is segment 1 (odd).
        Moment::Calm => (
            "Breathe",
            vec![
                step(
                    StepLen::Four,
                    (0..n).map(|i| if i % 2 == 0 { Role::Suave } else { Role::Off }),
                ),
                step(
                    StepLen::Four,
                    (0..n).map(|i| if i % 2 == 1 { Role::Suave } else { Role::Off }),
                ),
            ],
        ),
        Moment::BuildUp => (
            "Flash",
            vec![
                step(StepLen::Half, (0..n).map(|_| Role::Contraste)),
                step(StepLen::Half, (0..n).map(|_| Role::Off)),
            ],
        ),
    };
    Sequence {
        name: name.into(),
        moment,
        steps,
    }
}
