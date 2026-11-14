//! Seeded "Any pattern" picker: one of the moment's fixed patterns, never the same twice in a row.
use crate::detect::Moment;
use crate::model::Sequence;
use crate::patterns::{Pattern, build, for_moment};

/// Standard `SplitMix64` (Vigna). Hand-written to keep `light-core` dependency-free.
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (rejection sampling, no modulo bias). `below(0)` returns 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let min = n.wrapping_neg() % n;
        loop {
            let x = self.next_u64();
            if x >= min {
                return x % n;
            }
        }
    }
}

/// Index of `m` in `Generator::last`.
fn slot(m: Moment) -> usize {
    match m {
        Moment::Rhythm => 0,
        Moment::Calm => 1,
        Moment::BuildUp => 2,
    }
}

#[derive(Clone, Debug)]
pub struct Generator {
    rng: SplitMix64,
    last: [Option<Pattern>; 3],
}

impl Generator {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SplitMix64::new(seed),
            last: [None; 3],
        }
    }

    /// A random pattern of `m`, never the one this generator last gave for `m`.
    pub fn next(&mut self, m: Moment, n: u8) -> Sequence {
        let last = &mut self.last[slot(m)];
        let choices: Vec<Pattern> = for_moment(m)
            .iter()
            .copied()
            .filter(|p| Some(*p) != *last)
            .collect();
        let i = usize::try_from(self.rng.below(choices.len() as u64)).unwrap_or(0);
        let p = choices[i];
        *last = Some(p);
        build(p, n)
    }
}
