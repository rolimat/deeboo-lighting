#![cfg(feature = "serde")]
#![allow(clippy::expect_used)]
use light_core::library::{Interval, Library, Order, PaletteChange, SeqRef};
use light_core::migrate::{IntervalV1, LibraryV1, migrate_v1};
use light_core::model::StepLen;

const FIXTURE: &str = include_str!("fixtures/library-v1.json");

fn v1() -> LibraryV1 {
    serde_json::from_str(FIXTURE).expect("fixture parses as v1")
}

#[test]
fn migrates_a_real_v1_library() {
    let lib = migrate_v1(v1(), 8).expect("migrates");
    assert_eq!(lib.validate(), Ok(()));
    assert_eq!(lib.schema_version, 2);
    assert_eq!(lib.active_palette, None);
    assert_eq!(lib.active_set, 1);
    let club = &lib.sets[0];
    assert_eq!(club.name, "Club night");
    assert_eq!(club.palettes, vec![1, 2, 3, 4]);
    assert_eq!(club.palette_order, Order::Random);
    assert_eq!(club.palette_change, PaletteChange::AtDrop);
    assert_eq!(club.rhythm.interval, Interval::Bars(8));
    assert_eq!(club.rhythm.order, Order::Random);
    assert_eq!(
        club.rhythm.sequences,
        vec![SeqRef::Id(1), SeqRef::Id(4), SeqRef::Generated]
    );
    assert_eq!(club.calm.interval, Interval::OncePerBreak);
    assert_eq!(club.buildup.interval, Interval::Bars(2));
    let warm = &lib.sets[1];
    assert_eq!(warm.palettes, vec![3, 1]);
    assert_eq!(warm.palette_order, Order::InOrder);
    assert_eq!(warm.rhythm.interval, Interval::Bars(4));
    assert_eq!(warm.calm.interval, Interval::OncePerBreak);
    // The 3.5-beat "Skip" gained a half-beat step to fill its bar.
    let skip = &lib.sequences[3].1;
    assert_eq!(skip.steps.len(), 5);
    assert_eq!(skip.steps[4].len, StepLen::Half);
    assert_eq!(lib.sequences[0].1.steps.len(), 4, "whole bars untouched");
}

#[test]
fn phrase_interval_uses_phrase_bars() {
    for (pb, want) in [(4u8, 4u16), (8, 8), (16, 16)] {
        let lib = migrate_v1(v1(), pb).expect("migrates");
        assert_eq!(lib.sets[0].rhythm.interval, Interval::Bars(want));
    }
    let mut x = v1();
    x.sets[0].rhythm.interval = IntervalV1::Phrases(2);
    let lib = migrate_v1(x, 16).expect("migrates");
    assert_eq!(lib.sets[0].rhythm.interval, Interval::Bars(16), "capped");
}

#[test]
fn odd_beats_round_up_to_allowed_bars() {
    for (beats, want) in [
        (1u16, 1u16),
        (4, 1),
        (5, 2),
        (9, 4),
        (13, 4),
        (17, 8),
        (20, 8),
        (100, 16),
    ] {
        let mut x = v1();
        x.sets[0].buildup.interval = IntervalV1::Beats(beats);
        let lib = migrate_v1(x, 8).expect("migrates");
        assert_eq!(
            lib.sets[0].buildup.interval,
            Interval::Bars(want),
            "{beats} beats"
        );
    }
    let mut x = v1();
    x.sets[0].rhythm.interval = IntervalV1::Bars(3);
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.sets[0].rhythm.interval, Interval::Bars(4));
}

#[test]
fn pads_three_and_a_half_beats_to_one_bar_looping() {
    let lib = migrate_v1(v1(), 8).expect("migrates");
    let lens: Vec<_> = lib.sequences[3].1.steps.iter().map(|s| s.len).collect();
    assert_eq!(
        lens,
        [
            StepLen::Half,
            StepLen::One,
            StepLen::One,
            StepLen::One,
            StepLen::Half
        ]
    );
}

#[test]
fn fixed_with_many_palettes_becomes_in_order() {
    let mut x = v1();
    x.sets[0].rhythm.order = Order::Fixed;
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.sets[0].palette_order, Order::InOrder);
    let mut x = v1();
    x.sets[0].rhythm.order = Order::Fixed;
    let set = &mut x.sets[0];
    for m in [&mut set.rhythm, &mut set.calm, &mut set.buildup] {
        m.palettes = vec![2];
    }
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(
        lib.sets[0].palette_order,
        Order::Fixed,
        "one palette keeps Fixed"
    );
}

#[test]
fn a_library_that_cannot_validate_is_an_error() {
    let mut x = v1();
    let set = &mut x.sets[0];
    for m in [&mut set.rhythm, &mut set.calm, &mut set.buildup] {
        m.palettes.clear();
    }
    assert!(migrate_v1(x, 8).is_err());
}

fn half_beats(l: &Library, i: usize) -> u32 {
    l.sequences[i]
        .1
        .steps
        .iter()
        .map(|s| u32::from(s.len.half_beats()))
        .sum()
}

#[test]
fn truncates_sequences_over_sixteen_bars() {
    let mut x = v1();
    let step = x.sequences[0].1.steps[0].clone();
    x.sequences[0].1.steps = vec![
        light_core::model::Step {
            len: StepLen::Four,
            ..step
        };
        64
    ];
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.validate(), Ok(()));
    assert_eq!(half_beats(&lib, 0), 128);
    assert_eq!(lib.sequences[0].1.steps.len(), 16);
    assert!(
        lib.sequences[0]
            .1
            .steps
            .iter()
            .all(|s| s.len == StepLen::Four)
    );
}

#[test]
fn odd_long_sequence_pads_or_truncates_to_valid() {
    let mut x = v1();
    let step = x.sequences[0].1.steps[0].clone();
    x.sequences[0].1.steps = vec![
        light_core::model::Step {
            len: StepLen::Half,
            ..step.clone()
        };
        37
    ];
    // 127 half beats of Four then a Two: the crossing step is shortened to fit.
    let mut long = vec![
        light_core::model::Step {
            len: StepLen::Four,
            ..step.clone()
        };
        15
    ];
    long.push(light_core::model::Step {
        len: StepLen::Four,
        ..step.clone()
    });
    long.push(light_core::model::Step {
        len: StepLen::Two,
        ..step
    });
    x.sequences[1].1.steps = long;
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.validate(), Ok(()));
    assert_eq!(half_beats(&lib, 0), 40, "37 padded to 5 bars");
    assert_eq!(half_beats(&lib, 1), 128, "132 cut to 16 bars");
    assert_eq!(lib.sequences[1].1.steps.len(), 16);
}
