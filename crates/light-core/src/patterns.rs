//! The 35 fixed sequences (spec 2026-11-14 §3): 15 Rhythm, 10 Calm, 10 Build-up.
//!
//! Each is a table of `(length, base role, layers)`: the base fills the strip, then the layers paint
//! parts of it in order (the last one wins where they overlap). Parts come from `parts::parts(n)`.
#![allow(clippy::enum_glob_use)]
use crate::detect::Moment;
use crate::library::pad_to_bars;
use crate::model::{Role, Sequence, Step, StepLen};
use crate::parts::{Parts, parts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Pattern {
    // Rhythm
    CrossPairs,
    ColourAndDark,
    WhiteBackbeat,
    OutsideIn,
    HalvesBlink,
    CrossPairsWhiteBackbeat,
    ColourDarkWhite,
    OutsideInWithDark,
    MirrorWithWhite,
    ColourAndDarkTwoBars,
    EveryOtherBeat,
    HalvesWhiteBackbeat,
    OutsideInTwoBars,
    CrossPairsWhiteTurnaround,
    EndsCentreAllDark,
    // Calm
    WashAndSoften,
    EndsWhiteThenWash,
    SlowFadeToDark,
    ColourAndWhite,
    CentreToEnds,
    ColourCycle,
    FadeAndReturn,
    SlowWhiteBreath,
    LongWash,
    WhiteEndsSlowly,
    // Build-up
    Strobe,
    Converge,
    ColourStrobe,
    OddEvenStrobe,
    ConvergeThenStrobe,
    HalvesAccelerating,
    OddEvenAccelerating,
    StrobeAccelerating,
    ConvergeAccelerating,
    FourPartChaseAccelerating,
}

use Pattern::*;

const RHYTHM: [Pattern; 15] = [
    CrossPairs,
    ColourAndDark,
    WhiteBackbeat,
    OutsideIn,
    HalvesBlink,
    CrossPairsWhiteBackbeat,
    ColourDarkWhite,
    OutsideInWithDark,
    MirrorWithWhite,
    ColourAndDarkTwoBars,
    EveryOtherBeat,
    HalvesWhiteBackbeat,
    OutsideInTwoBars,
    CrossPairsWhiteTurnaround,
    EndsCentreAllDark,
];
const CALM: [Pattern; 10] = [
    WashAndSoften,
    EndsWhiteThenWash,
    SlowFadeToDark,
    ColourAndWhite,
    CentreToEnds,
    ColourCycle,
    FadeAndReturn,
    SlowWhiteBreath,
    LongWash,
    WhiteEndsSlowly,
];
const BUILDUP: [Pattern; 10] = [
    Strobe,
    Converge,
    ColourStrobe,
    OddEvenStrobe,
    ConvergeThenStrobe,
    HalvesAccelerating,
    OddEvenAccelerating,
    StrobeAccelerating,
    ConvergeAccelerating,
    FourPartChaseAccelerating,
];

impl Pattern {
    /// English name shown in the UI.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CrossPairs => "Cross pairs",
            ColourAndDark => "Colour and dark",
            WhiteBackbeat => "White backbeat",
            OutsideIn => "Outside in",
            HalvesBlink => "Halves blink",
            CrossPairsWhiteBackbeat => "Cross pairs + white backbeat",
            ColourDarkWhite => "Colour, dark, white",
            OutsideInWithDark => "Outside in, with dark",
            MirrorWithWhite => "Mirror with white",
            ColourAndDarkTwoBars => "Colour and dark, two bars",
            EveryOtherBeat => "Every other beat",
            HalvesWhiteBackbeat => "Halves + white backbeat",
            OutsideInTwoBars => "Outside in, two bars",
            CrossPairsWhiteTurnaround => "Cross pairs, white turnaround",
            EndsCentreAllDark => "Ends, centre, all, dark",
            WashAndSoften => "Wash and soften",
            EndsWhiteThenWash => "Ends white, then wash",
            SlowFadeToDark => "Slow fade to dark",
            ColourAndWhite => "Colour and white",
            CentreToEnds => "Centre to ends",
            ColourCycle => "Colour cycle",
            FadeAndReturn => "Fade and return",
            SlowWhiteBreath => "Slow white breath",
            LongWash => "Long wash",
            WhiteEndsSlowly => "White ends, slowly",
            Strobe => "Strobe",
            Converge => "Converge",
            ColourStrobe => "Colour strobe",
            OddEvenStrobe => "Odd / even strobe",
            ConvergeThenStrobe => "Converge, then strobe",
            HalvesAccelerating => "Halves, accelerating",
            OddEvenAccelerating => "Odd / even, accelerating",
            StrobeAccelerating => "Strobe, accelerating",
            ConvergeAccelerating => "Converge, accelerating",
            FourPartChaseAccelerating => "Four-part chase, accelerating",
        }
    }

    /// The moment this pattern is made for.
    #[must_use]
    pub fn moment(self) -> Moment {
        if RHYTHM.contains(&self) {
            Moment::Rhythm
        } else if CALM.contains(&self) {
            Moment::Calm
        } else {
            Moment::BuildUp
        }
    }
}

/// The patterns of `m`, in table order.
#[must_use]
pub fn for_moment(m: Moment) -> &'static [Pattern] {
    match m {
        Moment::Rhythm => &RHYTHM,
        Moment::Calm => &CALM,
        Moment::BuildUp => &BUILDUP,
    }
}

#[derive(Clone, Copy)]
enum Region {
    P1,
    P2,
    P3,
    P4,
    P13,
    P24,
    Ends,
    Centre,
    H1,
    H2,
    Odd,
    Even,
}

use Region::*;
use Role::{Contraste as C, Off as O, Principal as P, Suave as S, White as W};
use StepLen::{Four, Half, One};

type Layer = (Region, Role);
type Row = (StepLen, Role, &'static [Layer]);

fn region(p: &Parts, r: Region) -> [&[u8]; 2] {
    let none: &[u8] = &[];
    match r {
        P1 => [&p.p[0], none],
        P2 => [&p.p[1], none],
        P3 => [&p.p[2], none],
        P4 => [&p.p[3], none],
        P13 => [&p.p[0], &p.p[2]],
        P24 => [&p.p[1], &p.p[3]],
        Ends => [&p.ends, none],
        Centre => [&p.centre, none],
        H1 => [&p.h1, none],
        H2 => [&p.h2, none],
        Odd => [&p.odd, none],
        Even => [&p.even, none],
    }
}

fn paint(p: &Parts, n: u8, base: Role, layers: &[Layer]) -> Vec<Role> {
    let mut roles = vec![base; usize::from(n)];
    for (r, role) in layers {
        for &seg in region(p, *r).into_iter().flatten() {
            if let Some(slot) = usize::from(seg)
                .checked_sub(1)
                .and_then(|i| roles.get_mut(i))
            {
                *slot = *role;
            }
        }
    }
    roles
}

fn rep(rows: &[Row], times: usize) -> Vec<Row> {
    rows.iter()
        .copied()
        .cycle()
        .take(rows.len() * times)
        .collect()
}

fn at(rows: &[Row], len: StepLen) -> Vec<Row> {
    rows.iter().map(|&(_, b, l)| (len, b, l)).collect()
}

/// Calm: `(bars, base, layers)`, a step of k bars is k `Four` steps.
fn calm(rows: &[(usize, Role, &'static [Layer])]) -> Vec<Row> {
    rows.iter()
        .flat_map(|&(k, b, l)| std::iter::repeat_n((Four, b, l), k))
        .collect()
}

#[allow(clippy::too_many_lines)] // one data table
fn rows(p: Pattern) -> Vec<Row> {
    let strobe: [Row; 2] = [(Half, W, &[]), (Half, O, &[])];
    let converge: [Row; 4] = [
        (Half, O, &[(Ends, W)]),
        (Half, O, &[(Centre, P)]),
        (Half, C, &[]),
        (Half, O, &[]),
    ];
    let halves: [Row; 2] = [(One, O, &[(H1, W), (H2, P)]), (One, O, &[(H1, P), (H2, W)])];
    let odd_even: [Row; 2] = [
        (One, O, &[(Odd, W), (Even, P)]),
        (One, O, &[(Even, W), (Odd, P)]),
    ];
    let chase_p: [Row; 4] = [
        (One, O, &[(P1, P)]),
        (One, O, &[(P2, P)]),
        (One, O, &[(P3, P)]),
        (One, O, &[(P4, P)]),
    ];
    let chase_w: [Row; 4] = [
        (Half, O, &[(P1, W)]),
        (Half, O, &[(P2, W)]),
        (Half, O, &[(P3, W)]),
        (Half, O, &[(P4, W)]),
    ];
    match p {
        CrossPairs => vec![
            (One, O, &[(P13, P)]),
            (One, O, &[(P24, P)]),
            (One, O, &[(P13, P)]),
            (One, O, &[(P24, P)]),
            (One, O, &[(P13, C)]),
            (One, O, &[(P24, C)]),
            (One, O, &[(P13, C)]),
            (One, O, &[(P24, C)]),
        ],
        ColourAndDark => vec![(One, P, &[]), (One, O, &[]), (One, C, &[]), (One, O, &[])],
        WhiteBackbeat => vec![
            (One, O, &[(P13, P)]),
            (One, W, &[]),
            (One, O, &[(P24, C)]),
            (One, W, &[]),
        ],
        OutsideIn => vec![
            (One, O, &[(Ends, P)]),
            (One, O, &[(Centre, P)]),
            (One, O, &[(Ends, C)]),
            (One, O, &[(Centre, W)]),
        ],
        HalvesBlink => vec![
            (One, O, &[(H1, P)]),
            (One, O, &[(H2, W)]),
            (One, O, &[(H1, P)]),
            (One, O, &[(H2, W)]),
        ],
        CrossPairsWhiteBackbeat => vec![
            (One, O, &[(P13, P)]),
            (One, O, &[(P24, W)]),
            (One, O, &[(P13, C)]),
            (One, O, &[(P24, W)]),
        ],
        ColourDarkWhite => vec![(One, P, &[]), (One, O, &[]), (One, C, &[]), (One, W, &[])],
        OutsideInWithDark => vec![
            (One, O, &[(Ends, P)]),
            (One, O, &[]),
            (One, O, &[(Centre, P)]),
            (One, O, &[]),
            (One, O, &[(Ends, C)]),
            (One, O, &[]),
            (One, W, &[]),
            (One, O, &[]),
        ],
        MirrorWithWhite => vec![
            (One, O, &[(Ends, P)]),
            (One, O, &[(Centre, C)]),
            (One, O, &[(Ends, W)]),
            (One, O, &[]),
        ],
        ColourAndDarkTwoBars => vec![
            (One, P, &[]),
            (One, O, &[]),
            (One, C, &[]),
            (One, O, &[]),
            (One, S, &[]),
            (One, O, &[]),
            (One, W, &[]),
            (One, O, &[]),
        ],
        EveryOtherBeat => vec![
            (One, O, &[(P13, P)]),
            (One, O, &[]),
            (One, O, &[(P24, C)]),
            (One, O, &[]),
        ],
        HalvesWhiteBackbeat => vec![
            (One, O, &[(H1, P)]),
            (One, W, &[]),
            (One, O, &[(H2, C)]),
            (One, W, &[]),
        ],
        OutsideInTwoBars => vec![
            (One, O, &[(Ends, P)]),
            (One, O, &[(Centre, P)]),
            (One, O, &[(Ends, C)]),
            (One, O, &[(Centre, C)]),
            (One, O, &[(Ends, W)]),
            (One, O, &[]),
            (One, O, &[(Centre, W)]),
            (One, O, &[]),
        ],
        CrossPairsWhiteTurnaround => vec![
            (One, O, &[(P13, P)]),
            (One, O, &[(P24, P)]),
            (One, O, &[(P13, C)]),
            (One, W, &[]),
        ],
        EndsCentreAllDark => vec![
            (One, O, &[(Ends, P)]),
            (One, O, &[(Centre, C)]),
            (One, W, &[]),
            (One, O, &[]),
        ],
        WashAndSoften => calm(&[(2, P, &[]), (2, S, &[])]),
        EndsWhiteThenWash => calm(&[
            (2, O, &[(Ends, W)]),
            (2, S, &[]),
            (2, O, &[(Centre, P)]),
            (2, S, &[]),
        ]),
        SlowFadeToDark => calm(&[(2, P, &[]), (2, S, &[]), (2, O, &[(H1, S)]), (2, O, &[])]),
        ColourAndWhite => calm(&[(2, P, &[]), (2, C, &[]), (2, S, &[(Ends, W)]), (2, S, &[])]),
        CentreToEnds => calm(&[
            (2, S, &[(Centre, W)]),
            (2, P, &[]),
            (2, S, &[(Ends, W)]),
            (2, S, &[]),
        ]),
        ColourCycle => calm(&[
            (2, P, &[]),
            (2, C, &[]),
            (2, S, &[]),
            (2, P, &[(Centre, W)]),
        ]),
        FadeAndReturn => calm(&[
            (2, P, &[]),
            (2, S, &[]),
            (2, O, &[(Centre, S)]),
            (2, P, &[]),
        ]),
        SlowWhiteBreath => calm(&[(4, S, &[]), (2, W, &[]), (2, P, &[])]),
        LongWash => calm(&[(4, P, &[]), (4, S, &[])]),
        WhiteEndsSlowly => calm(&[(4, S, &[(Ends, W)]), (4, P, &[])]),
        Strobe => strobe.to_vec(),
        Converge => converge.to_vec(),
        ColourStrobe => vec![(Half, W, &[]), (Half, P, &[])],
        OddEvenStrobe => vec![(Half, O, &[(Odd, W)]), (Half, O, &[(Even, W)])],
        ConvergeThenStrobe => [rep(&converge, 2), rep(&strobe, 4)].concat(),
        HalvesAccelerating => [rep(&halves, 2), rep(&at(&halves, Half), 4)].concat(),
        OddEvenAccelerating => [
            rep(&odd_even, 2),
            rep(&at(&odd_even, Half), 2),
            rep(&strobe, 2),
        ]
        .concat(),
        StrobeAccelerating => [rep(&at(&strobe, One), 2), rep(&strobe, 4)].concat(),
        ConvergeAccelerating => [at(&converge, One), rep(&converge, 2)].concat(),
        FourPartChaseAccelerating => [chase_p.to_vec(), rep(&chase_w, 2)].concat(),
    }
}

/// Deterministic: the same `(p, n)` gives the same sequence (always fades, padded to whole bars).
#[must_use]
pub fn build(p: Pattern, n: u8) -> Sequence {
    let parts = parts(n);
    let mut seq = Sequence {
        name: p.label().into(),
        moment: p.moment(),
        fade: true,
        steps: rows(p)
            .into_iter()
            .map(|(len, base, layers)| Step {
                len,
                roles: paint(&parts, n, base, layers),
            })
            .collect(),
    };
    pad_to_bars(&mut seq);
    seq
}
