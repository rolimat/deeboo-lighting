#![allow(clippy::expect_used)]
use light_core::builtin::test_palette;
use light_core::detect::Moment;
use light_core::groups::groups;
use light_core::library::Shape;
use light_core::model::{Role, Sequence, StepLen, resolve};
use light_core::patterns::{Params, Pattern, build, for_moment};
use light_core::scene::diff;
use proptest::prelude::*;

const ALL: [Pattern; 11] = [
    Pattern::Halves,
    Pattern::EndsCentre,
    Pattern::OddEven,
    Pattern::Quarters,
    Pattern::Snake,
    Pattern::Mirror,
    Pattern::Drift,
    Pattern::Sparse,
    Pattern::GroupFlash,
    Pattern::FastSnake,
    Pattern::Strobe,
];

fn params(step: StepLen, density: u8, reverse: bool) -> Params {
    Params {
        step,
        roles: (Role::Principal, Role::Contraste),
        reverse,
        density,
    }
}

fn make(p: Pattern, pr: Params, n: u8) -> Sequence {
    build(p, &pr, &groups(&Shape::row(n)), n)
}

proptest! {
    #[test]
    fn every_pattern_valid_for_all_n(n in 1u8..=60, density in 0u8..=9, reverse: bool, s in 0usize..4) {
        let step = [StepLen::Half, StepLen::One, StepLen::Two, StepLen::Four][s];
        for p in ALL {
            let seq = make(p, params(step, density, reverse), n);
            prop_assert!(seq.validate(n).is_ok(), "{p:?} n={n}");
            prop_assert!(seq.steps.len() <= 64, "{p:?} n={n} steps={}", seq.steps.len());
        }
    }
}

#[test]
fn n_zero_does_not_panic() {
    for p in ALL {
        assert!(
            make(p, params(StepLen::One, 2, false), 0)
                .validate(0)
                .is_ok()
        );
    }
}

/// (one-message transitions, total transitions) incl. the wrap.
fn ratio(p: Pattern, n: u8) -> (usize, usize) {
    let pal = test_palette();
    let seq = make(p, params(StepLen::One, 2, false), n);
    let cols: Vec<Vec<_>> = seq
        .steps
        .iter()
        .map(|s| s.roles.iter().map(|r| resolve(*r, &pal, 100)).collect())
        .collect();
    if cols.len() < 2 {
        return (0, 0);
    }
    let one = (0..cols.len())
        .filter(|&i| diff(&cols[i], &cols[(i + 1) % cols.len()]).len() == 1)
        .count();
    (one, cols.len())
}

#[test]
fn one_message_preference() {
    let (mut one, mut all) = (0, 0);
    for p in ALL {
        let (mut po, mut pa) = (0, 0);
        for n in [6, 9, 10, 12, 16] {
            let (o, a) = ratio(p, n);
            (po, pa) = (po + o, pa + a);
        }
        eprintln!("{p:?}: {po}/{pa}");
        (one, all) = (one + po, all + pa);
    }
    assert!(one * 100 >= all * 80, "{one}/{all}");
}

#[test]
fn halves_matches_the_example() {
    let seq = make(Pattern::Halves, params(StepLen::One, 2, false), 8);
    assert_eq!(seq.name, "Halves");
    assert_eq!(seq.moment, Moment::Rhythm);
    let (p, c) = (Role::Principal, Role::Contraste);
    assert!(seq.steps.iter().all(|s| s.len == StepLen::One));
    assert!(
        seq.steps
            .iter()
            .any(|s| s.roles == [p, p, p, p, c, c, c, c])
    );
    assert!(
        seq.steps
            .iter()
            .any(|s| s.roles == [c, c, c, c, p, p, p, p])
    );
}

#[test]
fn calm_patterns_use_suave_and_slow_steps() {
    for p in [Pattern::Drift, Pattern::Sparse] {
        for n in [1, 6, 16, 60] {
            let seq = make(p, params(StepLen::Half, 3, false), n);
            assert_eq!(seq.moment, Moment::Calm);
            for s in &seq.steps {
                assert!(s.len.half_beats() >= 8, "{p:?}");
                assert!(s.roles.iter().all(|r| matches!(r, Role::Suave | Role::Off)));
            }
        }
    }
}

#[test]
fn buildup_patterns_are_fast() {
    for p in [Pattern::GroupFlash, Pattern::FastSnake, Pattern::Strobe] {
        let seq = make(p, params(StepLen::Four, 2, false), 12);
        assert_eq!(seq.moment, Moment::BuildUp);
        assert!(seq.steps.iter().all(|s| s.len.half_beats() <= 2), "{p:?}");
    }
}

#[test]
fn for_moment_lists_matching_patterns() {
    for m in [Moment::Rhythm, Moment::Calm, Moment::BuildUp] {
        assert_ne!(for_moment(m).len(), 0);
        for p in for_moment(m) {
            assert_eq!(make(*p, params(StepLen::One, 2, false), 8).moment, m);
        }
    }
}
