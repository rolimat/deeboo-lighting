//! Spatial groupings of the segments, derived from the strip shape (spec §6.4).
//! Positions are piece centres in editor coordinates (x grows right, y grows down, like
//! `ui/src/editor/strip/shape.ts`), so "left" and "top" match what the editor draws.
//! Splits are at the median; with odd N the first half (left/top/centre) gets the extra
//! segment; ties break by segment order. Every grouping partitions 1..=N exactly.
use crate::library::Shape;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Groups {
    pub left_right: [Vec<u8>; 2],
    pub top_bottom: [Vec<u8>; 2],
    pub centre_ends: [Vec<u8>; 2],
    pub odd_even: [Vec<u8>; 2],
    pub quarters: [Vec<u8>; 4],
    pub along: Vec<u8>,
}

/// Centre of the piece rectangle (same geometry as `rect` in shape.ts).
fn centre(p: &crate::library::Piece) -> (f32, f32) {
    let h = f32::from(p.len) / 2.0;
    match p.angle {
        90 => (p.x - 0.5, p.y + h),
        180 => (p.x - h, p.y - 0.5),
        270 => (p.x + 0.5, p.y - h),
        0 => (p.x + h, p.y + 0.5),
        a => {
            // The centre (h, 0.5) turned clockwise about the anchor.
            let (sin, cos) = f32::from(a).to_radians().sin_cos();
            (
                h.mul_add(cos, p.x) - 0.5 * sin,
                h.mul_add(sin, p.y) + 0.5 * cos,
            )
        }
    }
}

/// Segments (1-based) sorted by `key` (ties by segment order), split after the first ceil(n/2).
fn halves(keys: &[f32]) -> [Vec<u8>; 2] {
    let mut idx: Vec<usize> = (0..keys.len()).collect();
    idx.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]).then(a.cmp(&b)));
    let (a, b) = idx.split_at(keys.len().div_ceil(2));
    let seg = |s: &[usize]| {
        let mut v: Vec<u8> = s
            .iter()
            .map(|&i| u8::try_from(i + 1).unwrap_or(u8::MAX))
            .collect();
        v.sort_unstable();
        v
    };
    [seg(a), seg(b)]
}

#[must_use]
pub fn groups(shape: &Shape) -> Groups {
    let n8 = u8::try_from(shape.pieces.len()).unwrap_or(u8::MAX);
    let n = usize::from(n8);
    let cs: Vec<(f32, f32)> = shape.pieces[..n].iter().map(centre).collect();
    let xs: Vec<f32> = cs.iter().map(|c| c.0).collect();
    let ys: Vec<f32> = cs.iter().map(|c| c.1).collect();
    let (mx, my) = (
        xs.iter().sum::<f32>() / f32::from(n8.max(1)),
        ys.iter().sum::<f32>() / f32::from(n8.max(1)),
    );
    let dist: Vec<f32> = cs.iter().map(|c| (c.0 - mx).hypot(c.1 - my)).collect();
    let along: Vec<u8> = (1..=n8).collect();
    let parity = |r: usize| {
        along
            .iter()
            .copied()
            .filter(|s| usize::from(*s - 1) % 2 == r)
            .collect()
    };
    let q = |k: usize| along[k * n / 4..(k + 1) * n / 4].to_vec();
    Groups {
        left_right: halves(&xs),
        top_bottom: halves(&ys),
        centre_ends: halves(&dist),
        odd_even: [parity(0), parity(1)],
        quarters: [q(0), q(1), q(2), q(3)],
        along,
    }
}
