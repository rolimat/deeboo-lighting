use light_core::clock::Position;
use light_core::library::{Interval, MomentSet, Order, SeqRef};
use light_core::rotation::{Choice, Rotator};

#[allow(clippy::cast_possible_truncation)] // remainders < 16
fn pos(beat: u64, half: bool, phrase_bars: u8) -> Position {
    Position {
        beat,
        half,
        beat_in_bar: (beat % 4) as u8 + 1,
        bar_in_phrase: (beat / 4 % u64::from(phrase_bars)) as u8 + 1,
        phrase_bars,
    }
}

fn set(n_seq: u32, n_pal: u32, order: Order, interval: Interval) -> MomentSet {
    MomentSet {
        sequences: (1..=n_seq).map(SeqRef::Id).collect(),
        palettes: (1..=n_pal).collect(),
        order,
        interval,
    }
}

/// Runs bar starts 0..bars (phrase of 4 bars) and returns the bar indexes that changed.
fn changes(r: &mut Rotator, bars: u64) -> Vec<u64> {
    (0..bars)
        .filter(|b| r.on_boundary(pos(b * 4, false, 4), false).is_some())
        .collect()
}

#[test]
fn rhythm_changes_every_phrase_by_default() {
    let lib = light_core::library::Library::builtin(4);
    let mut ms = lib.current_set().unwrap().rhythm.clone();
    assert_eq!(ms.interval, Interval::Phrases(1));
    ms.sequences = vec![SeqRef::Id(1), SeqRef::Id(2)];
    let mut r = Rotator::new(&ms, 7);
    assert_eq!(changes(&mut r, 13), vec![4, 8, 12]);
}

#[test]
fn buildup_changes_every_2_bars() {
    let mut r = Rotator::new(&set(3, 1, Order::InOrder, Interval::Bars(2)), 7);
    assert_eq!(changes(&mut r, 7), vec![2, 4, 6]);
}

#[test]
fn calm_one_sequence_per_break() {
    let mut r = Rotator::new(&set(3, 1, Order::InOrder, Interval::Phrases(1)), 7).per_entry();
    assert_eq!(changes(&mut r, 20), Vec::<u64>::new());
    // The first entry plays the set's first sequence; later entries advance.
    assert_eq!(r.on_boundary(pos(40, false, 4), true), None);
    assert_eq!(
        r.on_boundary(pos(80, false, 4), true),
        Some(Choice::Sequence(SeqRef::Id(2)))
    );
    assert_eq!(changes(&mut r, 20), Vec::<u64>::new());
    assert_eq!(
        r.on_boundary(pos(160, false, 4), true),
        Some(Choice::Sequence(SeqRef::Id(3)))
    );
}

#[test]
fn change_waits_for_bar_boundary() {
    // Interval of one beat, but nothing happens off a bar start (or a phrase start for phrases).
    let mut r = Rotator::new(&set(3, 1, Order::InOrder, Interval::Beats(1)), 7);
    for beat in 0..16u64 {
        for half in [false, true] {
            let got = r.on_boundary(pos(beat, half, 4), false);
            let bar_start = beat % 4 == 0 && !half;
            assert!(bar_start || got.is_none(), "beat {beat} half {half}");
        }
    }
    let mut r = Rotator::new(&set(3, 1, Order::InOrder, Interval::Phrases(1)), 7);
    assert!(r.on_boundary(pos(0, false, 4), false).is_none());
    assert!(r.on_boundary(pos(12, false, 4), false).is_none());
    assert!(r.on_boundary(pos(16, false, 4), false).is_some());
}

#[test]
fn single_option_random_is_stable() {
    let mut r = Rotator::new(&set(1, 1, Order::Random, Interval::Bars(1)), 7);
    assert_eq!(changes(&mut r, 20), Vec::<u64>::new());
    assert_eq!(r.on_drop(), None);
}

#[test]
fn random_never_repeats_previous() {
    let mut r = Rotator::new(&set(3, 1, Order::Random, Interval::Bars(1)), 99);
    let mut prev = SeqRef::Id(1);
    for b in 0..200 {
        if let Some(Choice::Sequence(s)) = r.on_boundary(pos(b * 4, false, 4), false) {
            assert_ne!(s, prev);
            prev = s;
        }
    }
    let mut r = Rotator::new(&set(1, 3, Order::Random, Interval::Bars(1)), 5);
    let mut prev = 1;
    for _ in 0..200 {
        let p = r.on_drop().unwrap();
        assert_ne!(p, prev);
        prev = p;
    }
}

#[test]
fn fixed_never_changes() {
    let mut r = Rotator::new(&set(3, 3, Order::Fixed, Interval::Bars(1)), 7);
    assert_eq!(changes(&mut r, 20), Vec::<u64>::new());
    assert_eq!(r.on_drop(), None);
}

#[test]
fn new_palette_on_each_drop() {
    let mut r = Rotator::new(&set(1, 3, Order::InOrder, Interval::Phrases(1)), 7);
    assert_eq!(
        [r.on_drop(), r.on_drop(), r.on_drop()],
        [Some(2), Some(3), Some(1)]
    );
}

#[test]
fn lone_generated_is_new_each_interval() {
    let mut ms = set(1, 1, Order::InOrder, Interval::Bars(1));
    ms.sequences = vec![SeqRef::Generated];
    let mut r = Rotator::new(&ms, 7);
    assert_eq!(changes(&mut r, 4), vec![1, 2, 3]);
}

#[test]
fn beat_count_restart_does_not_freeze_the_interval() {
    let mut r = Rotator::new(&set(2, 1, Order::InOrder, Interval::Phrases(1)), 7);
    assert!(r.on_boundary(pos(2000, false, 4), true).is_none());
    // A MIDI Start restarts the beat count at 0.
    assert!(r.on_boundary(pos(0, false, 4), false).is_none());
    assert!(r.on_boundary(pos(16, false, 4), false).is_some());
}

#[test]
fn palette_position_starts_at_the_shown_palette() {
    let mut r =
        Rotator::new(&set(1, 3, Order::InOrder, Interval::Phrases(1)), 7).with_palette(Some(3));
    assert_eq!(r.on_drop(), Some(1));
    for seed in 0..20 {
        let mut r = Rotator::new(&set(1, 3, Order::Random, Interval::Phrases(1)), seed)
            .with_palette(Some(2));
        let mut prev = 2;
        for _ in 0..50 {
            let p = r.on_drop().unwrap();
            assert_ne!(p, prev);
            prev = p;
        }
    }
}
