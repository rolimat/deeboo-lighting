//! Colour mixing for the cross-fades between Calm steps.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names
)] // clamped values; max is exactly one of the channels

use crate::model::Colour;
use crate::paint::Hsv;

/// Frames of a fade for a step of `half_beats`: `min(4, half_beats / 2)`, at least 1 (1 = cut).
#[must_use]
pub fn frames(half_beats: u8) -> u8 {
    (half_beats / 2).clamp(1, 4)
}

fn to_rgb(c: Colour) -> [f32; 3] {
    let Some(Hsv { h, s, v }) = c else {
        return [0.0; 3];
    };
    let (v, s) = (f32::from(v) / 1000.0, f32::from(s) / 1000.0);
    let sector = f32::from(h % 360) / 60.0;
    let f = sector.fract();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match sector as u8 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn to_hsv([r, g, b]: [f32; 3]) -> Colour {
    let max = r.max(g).max(b);
    let d = max - r.min(g).min(b);
    let v = (max * 1000.0).round() as u16;
    if v == 0 {
        return None;
    }
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    Some(Hsv {
        h: (h.round() as u16) % 360,
        s: (d / max * 1000.0).round() as u16,
        v,
    })
}

/// `to` at `num/den` of the way from `from`, interpolated in RGB per channel and returned as HSV
/// (H 0-360, S/V 0-1000, rounded). `None` (off) is black. `num >= den` returns `to` exactly;
/// a mix with V == 0 is `None`.
#[must_use]
pub fn mix(from: Colour, to: Colour, num: u8, den: u8) -> Colour {
    if num >= den {
        return to;
    }
    let (a, b) = (to_rgb(from), to_rgb(to));
    let t = f32::from(num) / f32::from(den);
    to_hsv([0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t))
}
