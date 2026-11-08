use light_core::clock::{PhraseBars, Position};
use light_core::detect::Moment::{self, BuildUp, Calm, Rhythm};
use light_core::moments::MomentControl;

#[allow(clippy::cast_possible_truncation)] // remainders < 16
fn pos(beat: u64, phrase: PhraseBars) -> Position {
    Position {
        beat,
        half: false,
        beat_in_bar: (beat % 4) as u8 + 1,
        bar_in_phrase: (beat / 4 % u64::from(phrase.bars())) as u8 + 1,
        phrase_bars: phrase.bars(),
    }
}

const P: PhraseBars = PhraseBars::Four;

fn step(c: &mut MomentControl, beat: u64, d: Option<Moment>) -> Moment {
    c.next(pos(beat, P), P, d)
}

#[test]
fn calm_applies_from_the_next_beat() {
    let mut c = MomentControl::new();
    assert_eq!(step(&mut c, 1, Some(Calm)), Calm);
    assert_eq!(c.applied(), Calm);
    assert_eq!(step(&mut c, 2, Some(Rhythm)), Rhythm);
}

#[test]
fn buildup_waits_for_the_bar_start() {
    let mut c = MomentControl::new();
    assert_eq!(step(&mut c, 5, Some(BuildUp)), Rhythm);
    assert_eq!(step(&mut c, 6, Some(BuildUp)), Rhythm);
    assert_eq!(step(&mut c, 7, Some(BuildUp)), Rhythm);
    assert_eq!(step(&mut c, 8, Some(BuildUp)), BuildUp);
    assert_eq!(step(&mut c, 9, Some(BuildUp)), BuildUp);
}

#[test]
fn buildup_wait_cancelled_by_other_detection() {
    let mut c = MomentControl::new();
    assert_eq!(step(&mut c, 5, Some(BuildUp)), Rhythm);
    assert_eq!(step(&mut c, 6, Some(Calm)), Calm);
    assert_eq!(step(&mut c, 7, Some(Calm)), Calm);
    assert_eq!(step(&mut c, 8, Some(Calm)), Calm);
}

#[test]
fn drop_applies_immediately() {
    let mut c = MomentControl::new();
    assert_eq!(step(&mut c, 4, Some(BuildUp)), BuildUp);
    assert_eq!(step(&mut c, 5, Some(BuildUp)), BuildUp);
    assert_eq!(step(&mut c, 6, Some(Rhythm)), Rhythm);
    assert!(!c.predicted());
}

#[test]
fn predicted_drop_at_phrase_start_for_4_8_16() {
    for (phrase, start) in [
        (PhraseBars::Four, 16),
        (PhraseBars::Eight, 32),
        (PhraseBars::Sixteen, 64),
    ] {
        let mut c = MomentControl::new();
        assert_eq!(
            c.next(pos(start - 12, phrase), phrase, Some(BuildUp)),
            BuildUp
        );
        for b in start - 11..start {
            assert_eq!(
                c.next(pos(b, phrase), phrase, Some(BuildUp)),
                BuildUp,
                "{phrase:?} {b}"
            );
            assert!(!c.predicted());
        }
        assert_eq!(
            c.next(pos(start, phrase), phrase, Some(BuildUp)),
            Rhythm,
            "{phrase:?}"
        );
        assert!(c.predicted());
        assert_eq!(c.applied(), Rhythm);
    }
}

// The detector needs `rhythm_after_full` (2) full beats to report a drop, and `detect()` at a pulse
// describes the beat before: a real drop is reported 3 calls after the prediction call.
#[test]
fn unconfirmed_prediction_reverts_after_the_detector_lag() {
    let mut c = MomentControl::new();
    for b in 4..16 {
        step(&mut c, b, Some(BuildUp));
    }
    assert_eq!(step(&mut c, 16, Some(BuildUp)), Rhythm);
    assert!(c.predicted());
    assert_eq!(step(&mut c, 17, Some(BuildUp)), Rhythm);
    assert_eq!(step(&mut c, 18, Some(BuildUp)), Rhythm);
    assert!(c.predicted());
    assert_eq!(step(&mut c, 19, Some(BuildUp)), BuildUp);
    assert!(!c.predicted());
    assert_eq!(c.applied(), BuildUp);
}

#[test]
fn confirmed_prediction_stays_rhythm() {
    let mut c = MomentControl::new();
    for b in 4..16 {
        step(&mut c, b, Some(BuildUp));
    }
    assert_eq!(step(&mut c, 16, Some(BuildUp)), Rhythm);
    assert_eq!(step(&mut c, 17, Some(BuildUp)), Rhythm);
    assert_eq!(step(&mut c, 18, Some(Rhythm)), Rhythm);
    assert!(!c.predicted());
    assert_eq!(step(&mut c, 19, Some(Rhythm)), Rhythm);
}

#[test]
fn forced_ignores_detector_until_auto() {
    let mut c = MomentControl::new();
    c.force(Calm);
    assert!(!c.auto());
    assert_eq!(c.applied(), Calm);
    assert_eq!(step(&mut c, 1, Some(Rhythm)), Calm);
    assert_eq!(step(&mut c, 2, None), Calm);
    c.reset();
    assert!(!c.auto());
    assert_eq!(step(&mut c, 3, Some(BuildUp)), Calm);
    c.set_auto();
    assert!(c.auto());
    assert_eq!(step(&mut c, 4, Some(BuildUp)), BuildUp);
}

#[test]
fn no_audio_stays_rhythm() {
    let mut c = MomentControl::new();
    step(&mut c, 1, Some(Calm));
    assert_eq!(step(&mut c, 2, None), Rhythm);
    assert_eq!(c.applied(), Rhythm);
    c.reset();
    assert_eq!(c.applied(), Rhythm);
}
