//! Built-in palette and sequences (spec §6.3, §6.4).
use crate::detect::Moment;
use crate::groups::Groups;
use crate::library::{Interval, MomentSet, Order, PaletteChange, SeqRef, Set};
use crate::model::{Palette, Role, Sequence, Step, StepLen};
use crate::paint::Hsv;
use crate::patterns::{Params, Pattern, build};

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

/// Eight palettes: Principal/Contraste at least 60 degrees apart, Suave light (S <= 450, V >= 800).
#[must_use]
pub fn palettes() -> Vec<Palette> {
    let p = |name: &str, a: (u16, u16, u16), b: (u16, u16, u16), c: (u16, u16, u16)| {
        let hsv = |(h, s, v)| Hsv { h, s, v };
        Palette {
            name: name.into(),
            principal: hsv(a),
            contraste: hsv(b),
            suave: hsv(c),
        }
    };
    vec![
        p("Sunset", (20, 950, 1000), (280, 900, 950), (30, 300, 1000)),
        p(
            "Ocean",
            (190, 1000, 1000),
            (320, 900, 1000),
            (190, 300, 1000),
        ),
        p(
            "Club",
            (300, 1000, 1000),
            (180, 1000, 1000),
            (300, 250, 1000),
        ),
        p(
            "Fire and ice",
            (10, 1000, 1000),
            (210, 1000, 1000),
            (20, 300, 1000),
        ),
        p("Forest", (120, 900, 900), (300, 800, 1000), (120, 300, 950)),
        p(
            "Aurora",
            (160, 900, 1000),
            (270, 900, 1000),
            (200, 250, 1000),
        ),
        p(
            "Candy",
            (330, 900, 1000),
            (180, 800, 1000),
            (330, 250, 1000),
        ),
        p("Gold", (45, 1000, 1000), (240, 900, 1000), (45, 300, 1000)),
    ]
}

/// Ten sequences for the strip: 4 Rhythm, 3 Calm, 3 Build-up, fixed params.
#[must_use]
pub fn sequences(n: u8, groups: &Groups) -> Vec<Sequence> {
    let mk = |pat, step, roles, reverse, density| {
        let params = Params {
            step,
            roles,
            reverse,
            density,
        };
        build(pat, &params, groups, n)
    };
    let (pc, cp) = (
        (Role::Principal, Role::Contraste),
        (Role::Contraste, Role::Principal),
    );
    let calm = (Role::Suave, Role::Off);
    let mut drift_back = mk(Pattern::Drift, StepLen::Four, calm, true, 2);
    drift_back.name = "Drift back".into();
    vec![
        mk(Pattern::Halves, StepLen::One, pc, false, 1),
        mk(Pattern::EndsCentre, StepLen::Two, cp, false, 1),
        mk(Pattern::Snake, StepLen::One, pc, false, 2),
        mk(Pattern::Quarters, StepLen::Two, pc, true, 1),
        mk(Pattern::Drift, StepLen::Four, calm, false, 1),
        mk(Pattern::Sparse, StepLen::Four, calm, false, 1),
        drift_back,
        mk(
            Pattern::GroupFlash,
            StepLen::Half,
            (Role::Contraste, Role::Off),
            false,
            1,
        ),
        mk(
            Pattern::FastSnake,
            StepLen::Half,
            (Role::White, Role::Off),
            false,
            2,
        ),
        mk(
            Pattern::Strobe,
            StepLen::Half,
            (Role::White, Role::Off),
            false,
            1,
        ),
    ]
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
