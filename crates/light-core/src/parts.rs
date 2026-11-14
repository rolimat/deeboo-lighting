//! The strip partition along segments 1..N (1-based, ascending sets), computed from `n` only.

/// Segment sets, 1-based, ascending. Every field is computed from `n` only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parts {
    pub p: [Vec<u8>; 4],
    pub ends: Vec<u8>,
    pub centre: Vec<u8>,
    pub h1: Vec<u8>,
    pub h2: Vec<u8>,
    pub odd: Vec<u8>,
    pub even: Vec<u8>,
    pub all: Vec<u8>,
}

/// `a..=b` as segments; u16 inside so `n + 1` never overflows, values are always <= n <= 255.
fn rng(a: u16, b: u16) -> Vec<u8> {
    (a..=b).filter_map(|s| u8::try_from(s).ok()).collect()
}

fn union(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = a.iter().chain(b).copied().collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[must_use]
pub fn parts(n: u8) -> Parts {
    let n = u16::from(n);
    let all = rng(1, n);
    let odd_n = n % 2 == 1;
    let mid = n.div_ceil(2); // middle segment of an odd strip
    let (p, ends, centre) = if n == 0 {
        (Default::default(), vec![], vec![])
    } else if n % 4 == 0 {
        let quarter = n / 4;
        let p = [
            rng(1, quarter),
            rng(quarter + 1, 2 * quarter),
            rng(2 * quarter + 1, 3 * quarter),
            rng(3 * quarter + 1, n),
        ];
        let (ends, centre) = (union(&p[0], &p[3]), union(&p[1], &p[2]));
        (p, ends, centre)
    } else if n % 3 == 0 {
        let third = n / 3;
        let p = [
            rng(1, third),
            rng(third + 1, 2 * third),
            rng(2 * third + 1, n),
            rng(third + 1, 2 * third),
        ];
        let ends = union(&p[0], &p[2]);
        let centre = p[1].clone();
        (p, ends, centre)
    } else {
        let even_n = n - u16::from(odd_n);
        let base = even_n / 4;
        let extra = (even_n - 4 * base) / 2;
        let hi2 = 2 * base + extra;
        let (p2, p3) = if odd_n {
            (rng(base + 1, hi2 + 1), rng(hi2 + 1, hi2 + 1 + base + extra))
        } else {
            (rng(base + 1, hi2), rng(hi2 + 1, hi2 + base + extra))
        };
        let p = [rng(1, base), p2, p3, rng(n - base + 1, n)];
        let (ends, centre) = (union(&p[0], &p[3]), union(&p[1], &p[2]));
        (p, ends, centre)
    };
    let (h1, h2) = if odd_n {
        (rng(1, mid), rng(mid, n))
    } else {
        (rng(1, n / 2), rng(n / 2 + 1, n))
    };
    let by_parity = |rem: u16| {
        all.iter()
            .copied()
            .filter(|seg| u16::from(*seg) % 2 == rem)
            .collect()
    };
    Parts {
        p,
        ends,
        centre,
        h1,
        h2,
        odd: by_parity(1),
        even: by_parity(0),
        all: all.clone(),
    }
}
