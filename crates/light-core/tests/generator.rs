use light_core::detect::Moment;
use light_core::generator::{Generator, SplitMix64};
use light_core::groups::groups;
use light_core::library::Shape;
use light_core::patterns::for_moment;

const MOMENTS: [Moment; 3] = [Moment::Rhythm, Moment::Calm, Moment::BuildUp];

#[test]
fn splitmix_known_values() {
    // Reference outputs of the standard SplitMix64 (Vigna, prng.di.unimi.it/splitmix64.c)
    // for seed 1234567, cross-checked with an independent implementation.
    let mut r = SplitMix64::new(1_234_567);
    assert_eq!(r.next_u64(), 6_457_827_717_110_365_317);
    assert_eq!(r.next_u64(), 3_203_168_211_198_807_973);
    assert_eq!(r.next_u64(), 9_817_491_932_198_370_423);
}

#[test]
fn below_is_in_range_and_zero_is_safe() {
    let mut r = SplitMix64::new(7);
    assert!((0..1000).all(|_| r.below(5) < 5));
    assert_eq!(r.below(0), 0);
    assert_eq!(r.below(1), 0);
}

#[test]
fn same_seed_same_sequence() {
    let g = groups(&Shape::row(10));
    let (mut a, mut b) = (Generator::new(42), Generator::new(42));
    for i in 0..30 {
        let m = MOMENTS[i % 3];
        assert_eq!(a.next(m, &g, 10), b.next(m, &g, 10));
    }
}

#[test]
fn never_repeats_twice_in_a_row() {
    let g = groups(&Shape::row(12));
    for seed in 0..20 {
        let mut generator = Generator::new(seed);
        for m in MOMENTS {
            let mut prev = generator.next(m, &g, 12);
            for _ in 0..50 {
                let s = generator.next(m, &g, 12);
                assert_ne!(s, prev, "{m:?} seed {seed}");
                assert_ne!(s.name, prev.name, "pattern must alternate");
                assert!(s.steps.len() <= 64);
                prev = s;
            }
        }
    }
}

#[test]
fn covers_all_patterns_over_100_calls() {
    let g = groups(&Shape::row(12));
    for m in MOMENTS {
        let mut generator = Generator::new(1);
        let names: std::collections::HashSet<String> =
            (0..100).map(|_| generator.next(m, &g, 12).name).collect();
        assert_eq!(names.len(), for_moment(m).len(), "{m:?}");
    }
}
