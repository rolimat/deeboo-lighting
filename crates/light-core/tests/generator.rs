use light_core::detect::Moment;
use light_core::generator::{Generator, SplitMix64};
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
    let (mut a, mut b) = (Generator::new(42), Generator::new(42));
    for i in 0..30 {
        let m = MOMENTS[i % 3];
        assert_eq!(a.next(m, 10), b.next(m, 10));
    }
}

#[test]
fn generated_never_repeats_per_moment() {
    for seed in 0..5 {
        for m in MOMENTS {
            let mut generator = Generator::new(seed);
            let mut prev = generator.next(m, 8).name;
            for _ in 0..200 {
                let s = generator.next(m, 8);
                assert_eq!(s.moment, m);
                assert_ne!(s.name, prev, "{m:?} seed {seed}");
                prev = s.name;
            }
        }
    }
}

#[test]
fn memory_is_per_moment() {
    // Interleaving Calm picks must not reset Rhythm's memory.
    for seed in 0..20 {
        let mut generator = Generator::new(seed);
        let mut prev = generator.next(Moment::Rhythm, 8).name;
        for _ in 0..100 {
            generator.next(Moment::Calm, 8);
            let s = generator.next(Moment::Rhythm, 8);
            assert_ne!(s.name, prev, "seed {seed}");
            prev = s.name;
        }
    }
}

#[test]
fn covers_all_patterns_over_300_calls() {
    for m in MOMENTS {
        let mut generator = Generator::new(1);
        let names: std::collections::HashSet<String> =
            (0..300).map(|_| generator.next(m, 8).name).collect();
        assert_eq!(names.len(), for_moment(m).len(), "{m:?}");
    }
}
