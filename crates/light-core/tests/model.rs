use light_core::builtin::{sequence, test_palette};
use light_core::detect::Moment;
use light_core::model::{Role, Step, StepLen, resolve};
use light_core::paint::Hsv;

#[test]
fn resolve_white_off_and_brightness() {
    let p = test_palette();
    assert_eq!(
        resolve(Role::Principal, &p, 50),
        Some(Hsv {
            h: 0,
            s: 1000,
            v: 500
        })
    );
    assert_eq!(
        resolve(Role::White, &p, 100),
        Some(Hsv {
            h: 0,
            s: 0,
            v: 1000
        })
    );
    assert_eq!(resolve(Role::Off, &p, 100), None);
}

#[test]
fn brightness_is_clamped() {
    let p = test_palette();
    assert_eq!(resolve(Role::Principal, &p, 0).map(|c| c.v), Some(100));
    assert_eq!(resolve(Role::Principal, &p, 250).map(|c| c.v), Some(1000));
}

#[test]
fn builtin_sequences_validate_for_10_and_odd_counts() {
    for n in [10u8, 7] {
        for m in [Moment::Rhythm, Moment::Calm, Moment::BuildUp] {
            assert_eq!(sequence(m, n).validate(n), Ok(()), "{m:?} {n}");
        }
    }
}

#[test]
fn validate_rejects_wrong_lengths_and_empty() {
    let mut s = sequence(Moment::Rhythm, 10);
    assert!(s.validate(9).is_err());
    s.steps = vec![];
    assert!(s.validate(10).is_err());
    s.steps = vec![Step {
        len: StepLen::One,
        roles: vec![Role::Off; 3],
    }];
    assert!(s.validate(10).is_err());
}

#[test]
fn rhythm_halves_matches_the_example() {
    let s = sequence(Moment::Rhythm, 10);
    assert_eq!(s.steps[0].len.half_beats(), 2);
    assert_eq!(&s.steps[0].roles[..5], &[Role::Principal; 5]);
    assert_eq!(&s.steps[0].roles[5..], &[Role::Off; 5]);
    assert_eq!(&s.steps[1].roles[..5], &[Role::Off; 5]);
    assert_eq!(&s.steps[1].roles[5..], &[Role::White; 5]);
}
