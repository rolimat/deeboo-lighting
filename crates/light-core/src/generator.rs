//! Seeded sequence generator (spec §6.4): alternates patterns and parameters, deterministic per seed.
use crate::detect::Moment;
use crate::groups::Groups;
use crate::model::{Role, Sequence, StepLen};
use crate::patterns::{Params, Pattern, build, for_moment};

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

const STEPS: [StepLen; 4] = [StepLen::Half, StepLen::One, StepLen::Two, StepLen::Four];
const ROLES: [(Role, Role); 4] = [
    (Role::Principal, Role::Contraste),
    (Role::Contraste, Role::Principal),
    (Role::Principal, Role::Suave),
    (Role::Contraste, Role::Suave),
];

#[derive(Clone, Debug)]
pub struct Generator {
    rng: SplitMix64,
    last: Option<Pattern>,
}

impl Generator {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SplitMix64::new(seed),
            last: None,
        }
    }

    /// A pattern of `m` different from the previous one (every moment has >= 2),
    /// with random params. Alternating patterns guarantees (pattern, params) never repeats.
    pub fn next(&mut self, m: Moment, groups: &Groups, n: u8) -> Sequence {
        let all = for_moment(m);
        let choices: Vec<Pattern> = all
            .iter()
            .copied()
            .filter(|p| all.len() < 2 || Some(*p) != self.last)
            .collect();
        let pick =
            |rng: &mut SplitMix64, len: usize| usize::try_from(rng.below(len as u64)).unwrap_or(0);
        let p = choices[pick(&mut self.rng, choices.len())];
        let params = Params {
            step: STEPS[pick(&mut self.rng, STEPS.len())],
            roles: ROLES[pick(&mut self.rng, ROLES.len())],
            reverse: self.rng.below(2) == 1,
            density: 1 + u8::try_from(self.rng.below(4)).unwrap_or(0),
        };
        self.last = Some(p);
        build(p, &params, groups, n)
    }
}
