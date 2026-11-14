//! Built-in palette and sequences (spec §6.3, §6.4).
use crate::detect::Moment;
use crate::library::{Interval, MomentSet, Order, PaletteChange, SeqRef, Set};
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
        fade: true,
        moment,
        steps,
    }
}

/// Nine palettes (HSV, 0-360 / 0-1000): Principal and Contraste clearly apart, Suave light.
#[must_use]
pub fn palettes() -> Vec<Palette> {
    let hsv = |(h, s, v)| Hsv { h, s, v };
    [
        (
            "Ocean",
            (190, 1000, 1000),
            (320, 900, 1000),
            (190, 300, 1000),
        ),
        (
            "Laser",
            (125, 1000, 1000),
            (325, 1000, 1000),
            (160, 600, 1000),
        ),
        ("UV", (257, 1000, 1000), (177, 1000, 1000), (282, 500, 1000)),
        (
            "Deep sea",
            (197, 1000, 1000),
            (157, 1000, 1000),
            (182, 400, 1000),
        ),
        ("Ember", (28, 1000, 1000), (53, 1000, 1000), (43, 500, 1000)),
        (
            "Green",
            (130, 1000, 550),
            (130, 1000, 1000),
            (130, 450, 1000),
        ),
        (
            "Lavender",
            (270, 550, 1000),
            (220, 450, 1000),
            (220, 250, 1000),
        ),
        (
            "Sorbet",
            (45, 650, 1000),
            (350, 650, 1000),
            (350, 300, 1000),
        ),
        (
            "Peach and mint",
            (20, 550, 1000),
            (150, 550, 1000),
            (40, 250, 1000),
        ),
    ]
    .map(|(name, a, b, c)| Palette {
        name: name.into(),
        principal: hsv(a),
        contraste: hsv(b),
        suave: hsv(c),
    })
    .to_vec()
}

/// "Auto": a freshly generated sequence for every moment; the set's palettes rotate in order at
/// each drop. Rhythm every 8 bars, Calm once per break, Build-up every 2 bars.
#[must_use]
pub fn auto_set(id: u32, palettes: &[u32]) -> Set {
    let m = |interval| MomentSet {
        sequences: vec![SeqRef::Generated],
        order: Order::InOrder,
        interval,
    };
    Set {
        id,
        name: "Auto".into(),
        rhythm: m(Interval::Bars(8)),
        calm: m(Interval::OncePerBreak),
        buildup: m(Interval::Bars(2)),
        palettes: palettes.to_vec(),
        palette_order: Order::InOrder,
        palette_change: PaletteChange::AtDrop,
    }
}
