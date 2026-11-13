//! Generates the menu bar PNGs (18 px tall, @1x and @2x): the fixed 6-segment LED glyph, with the
//! moment icon (and an "F" badge when forced) beside it. Output is committed:
//! `cargo run -p tray-frames [dir]` (default `src-tauri/icons/tray`). Deterministic.

use std::path::Path;

const H: f32 = 18.0;
/// Glyph only (stopped).
const W_GLYPH: usize = 19;
/// Glyph + moment icon + "F" badge.
const W_FULL: usize = 37;

type Rgb = [u8; 3];
/// (colour, signed distance to the shape edge, opacity).
type Shape = Box<dyn Fn(f32, f32) -> (Rgb, f32, f32)>;

const BLACK: Rgb = [0, 0, 0]; // template images: only alpha matters
const GREEN: Rgb = [45, 212, 191]; // the app accent (`--color-accent`), lights on; not a template
const GREY: Rgb = [142, 142, 147]; // mid grey: legible on light and dark bars (non-template lost variants)
const RED: Rgb = [255, 69, 58];

/// Distance from `(px, py)` to the polyline.
fn dist_line(pts: &[(f32, f32)], px: f32, py: f32) -> f32 {
    pts.windows(2)
        .map(|w| {
            let (ax, ay) = w[0];
            let (bx, by) = w[1];
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (px - ax - t * dx).hypot(py - ay - t * dy)
        })
        .fold(f32::MAX, f32::min)
}

/// Points of a cubic Bezier, excluding the start point.
fn cubic(p0: (f32, f32), c1: (f32, f32), c2: (f32, f32), p1: (f32, f32)) -> Vec<(f32, f32)> {
    (1_u8..=16)
        .map(|i| {
            let t = f32::from(i) / 16.0;
            let u = 1.0 - t;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
            };
            (f(p0.0, c1.0, c2.0, p1.0), f(p0.1, c1.1, c2.1, p1.1))
        })
        .collect()
}

/// The moment icons as polylines in the 24x24 line-icon space (the SVG paths of the spec).
fn moment_paths(kind: &str) -> Vec<Vec<(f32, f32)>> {
    match kind {
        // M3 12h3l3-7 4 14 3-7h5
        "rhythm" => vec![vec![
            (3.0, 12.0),
            (6.0, 12.0),
            (9.0, 5.0),
            (13.0, 19.0),
            (16.0, 12.0),
            (21.0, 12.0),
        ]],
        // M2 12c3-4 5-4 8 0s5 4 8 0 3-2 4-1
        "calm" => {
            let mut p = vec![(2.0, 12.0)];
            p.extend(cubic((2.0, 12.0), (5.0, 8.0), (7.0, 8.0), (10.0, 12.0)));
            p.extend(cubic(
                (10.0, 12.0),
                (13.0, 16.0),
                (15.0, 16.0),
                (18.0, 12.0),
            ));
            p.extend(cubic((18.0, 12.0), (21.0, 8.0), (21.0, 10.0), (22.0, 11.0)));
            vec![p]
        }
        // M3 20 21 4M21 4v7M21 4h-7
        _ => vec![
            vec![(3.0, 20.0), (21.0, 4.0)],
            vec![(21.0, 4.0), (21.0, 11.0)],
            vec![(21.0, 4.0), (14.0, 4.0)],
        ],
    }
}

/// Straight-alpha RGBA, `scale` px per logical px.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]
fn render(scale: usize, width: usize, shapes: &[Shape]) -> Vec<u8> {
    let (w, h) = (width * scale, (H as usize) * scale);
    let mut out = Vec::with_capacity(w * h * 4);
    for py in 0..h {
        for px in 0..w {
            let (x, y) = (
                (px as f32 + 0.5) / scale as f32,
                (py as f32 + 0.5) / scale as f32,
            );
            // Source-over of every shape, in premultiplied floats.
            let (mut c, mut a) = ([0.0_f32; 3], 0.0_f32);
            for s in shapes {
                let (rgb, d, op) = s(x, y);
                let sa = (0.5 - d * scale as f32).clamp(0.0, 1.0) * op;
                for i in 0..3 {
                    c[i] = f32::from(rgb[i]) * sa + c[i] * (1.0 - sa);
                }
                a = sa + a * (1.0 - sa);
            }
            for v in c {
                out.push(if a > 0.0 { (v / a).round() as u8 } else { 0 });
            }
            out.push((a * 255.0).round() as u8);
        }
    }
    out
}

fn encode(scale: usize, width: usize, rgba: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        #[allow(clippy::cast_possible_truncation)] // 66 at most
        let mut enc = png::Encoder::new(&mut buf, (width * scale) as u32, (18 * scale) as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        // Fixed settings keep the bytes identical run to run.
        enc.set_compression(png::Compression::Balanced);
        if let Ok(mut w) = enc.write_header() {
            let _ = w.write_image_data(rgba);
        }
    }
    buf
}

/// Rounded bar `[x0,x1] x [y0,y1]`.
fn bar(x0: f32, y0: f32, x1: f32, y1: f32, rgb: Rgb) -> Shape {
    let (cx, cy, hx, hy) = (
        f32::midpoint(x0, x1),
        f32::midpoint(y0, y1),
        (x1 - x0) / 2.0,
        (y1 - y0) / 2.0,
    );
    let r = hy.min(hx); // full pill
    Box::new(move |x, y| {
        let (qx, qy) = ((x - cx).abs() - (hx - r), (y - cy).abs() - (hy - r));
        (
            rgb,
            qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r,
            1.0,
        )
    })
}

/// The mockup glyph (22x15 viewBox: 5x3 pills, rx 1.5, at x = 1 / 8.5 / 16 and y = 2 / 10), scaled 0.8
/// and centred vertically. Never animated.
fn glyph(rgb: Rgb) -> Vec<Shape> {
    const K: f32 = 0.8;
    let mut out = Vec::new();
    for y in [2.0_f32, 10.0] {
        for x in [1.0_f32, 8.5, 16.0] {
            let (x0, y0) = (1.0 + x * K, 3.0 + y * K);
            out.push(bar(x0, y0, x0 + 5.0 * K, y0 + 3.0 * K, rgb));
        }
    }
    out
}

/// The moment icon (24-unit space scaled to 12 px) at x = 20.
fn moment_icon(kind: &str, rgb: Rgb) -> Shape {
    let paths: Vec<Vec<(f32, f32)>> = moment_paths(kind)
        .into_iter()
        .map(|p| {
            p.into_iter()
                .map(|(x, y)| (20.0 + x * 0.5, 3.0 + y * 0.5))
                .collect()
        })
        .collect();
    Box::new(move |x, y| {
        let d = paths
            .iter()
            .map(|p| dist_line(p, x, y))
            .fold(f32::MAX, f32::min);
        (rgb, d - 0.65, 1.0)
    })
}

/// A small "F" (4 x 7 px) at the right of the icon.
fn forced_badge(rgb: Rgb) -> Shape {
    let strokes = [
        vec![(32.5, 5.5), (32.5, 12.5)],
        vec![(32.5, 5.5), (35.5, 5.5)],
        vec![(32.5, 8.7), (35.0, 8.7)],
    ];
    Box::new(move |x, y| {
        let d = strokes
            .iter()
            .map(|p| dist_line(p, x, y))
            .fold(f32::MAX, f32::min);
        (rgb, d - 0.55, 1.0)
    })
}

/// Variant name suffix and the shapes of one image.
fn image(kind: Option<&str>, forced: bool, on: bool, lost: bool) -> (usize, Vec<Shape>) {
    let ink = match (on, lost) {
        (true, _) => GREEN,
        (false, true) => GREY,
        (false, false) => BLACK,
    };
    let mut shapes = glyph(ink);
    if lost {
        // Top-right corner of the glyph, over the bars.
        shapes.push(Box::new(|x, y| (RED, (x - 16.8).hypot(y - 4.2) - 2.6, 1.0)));
    }
    let Some(kind) = kind else {
        return (W_GLYPH, shapes);
    };
    shapes.push(moment_icon(kind, ink));
    if forced {
        shapes.push(forced_badge(ink));
    }
    (W_FULL, shapes)
}

/// Every image as (file name, PNG bytes).
fn frames() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for scale in [1, 2] {
        let at = if scale == 1 { "" } else { "@2x" };
        for lost in [false, true] {
            let l = if lost { "-lost" } else { "" };
            for on in [false, true] {
                let o = if on { "-on" } else { "" };
                let (w, shapes) = image(None, false, on, lost);
                out.push((
                    format!("tray-stopped{o}{l}{at}.png"),
                    encode(scale, w, &render(scale, w, &shapes)),
                ));
                for kind in ["rhythm", "calm", "buildup"] {
                    for forced in [false, true] {
                        let name = format!(
                            "tray-{kind}{}{o}{l}{at}.png",
                            if forced { "-forced" } else { "" }
                        );
                        let (w, shapes) = image(Some(kind), forced, on, lost);
                        out.push((name, encode(scale, w, &render(scale, w, &shapes))));
                    }
                }
            }
        }
    }
    out
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "src-tauri/icons/tray".into());
    std::fs::create_dir_all(&dir)?;
    for (name, bytes) in frames() {
        std::fs::write(Path::new(&dir).join(name), bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut r = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let i = r.next_frame(&mut buf).unwrap();
        (i.width, i.height, buf)
    }

    fn get(all: &[(String, Vec<u8>)], name: &str) -> (u32, u32, Vec<u8>) {
        decode(
            &all.iter()
                .find(|(n, _)| n == name)
                .unwrap_or_else(|| panic!("{name} missing"))
                .1,
        )
    }

    fn alpha_sum(b: &[u8]) -> u32 {
        b.chunks(4).map(|p| u32::from(p[3])).sum()
    }

    #[test]
    fn tray_frames_tool_is_deterministic() {
        let a = frames();
        // (3 moments x forced + stopped) x on x lost, at two scales
        assert_eq!(a.len(), 2 * 4 * (3 * 2 + 1));
        assert_eq!(a, frames());
        assert!(
            a.iter()
                .any(|(n, _)| n == "tray-buildup-forced-on-lost@2x.png")
        );
        assert!(a.iter().any(|(n, _)| n == "tray-stopped.png"));
        assert!(
            !a.iter().any(|(n, _)| n.starts_with("dot-")
                || n.starts_with("rhythm-")
                || n.contains("-dim"))
        );
    }

    /// Template images (lights off, not lost): every pixel is black, only alpha varies. Lost ones carry a red dot; lights-on ones are green.
    #[test]
    fn tray_template_is_monochrome_and_lost_has_red() {
        let all = frames();
        for (name, bytes) in &all {
            let (_, _, buf) = decode(bytes);
            assert!(buf.chunks(4).any(|p| p[3] > 0), "{name} is empty");
            let red = buf
                .chunks(4)
                .any(|p| p[3] > 200 && p[0] > 200 && p[1] < 100);
            let green = buf.chunks(4).any(|p| p[3] > 200 && p[..3] == GREEN);
            assert_eq!(green, name.contains("-on"), "{name} green");
            if name.contains("-lost") {
                assert!(red, "{name} has no red dot");
            } else if name.contains("-on") {
                assert!(!red, "{name}");
            } else {
                assert!(
                    !red && buf.chunks(4).all(|p| p[..3] == [0, 0, 0]),
                    "{name} is not black + alpha"
                );
            }
        }
    }

    #[test]
    fn heights_widths_and_scales() {
        let all = frames();
        let (w1, h1, _) = get(&all, "tray-rhythm.png");
        let (w2, h2, _) = get(&all, "tray-rhythm@2x.png");
        assert_eq!((h1, h2, w2), (18, 36, 2 * w1));
        assert!(w1 <= 38, "compact: {w1}");
        let (ws, _, _) = get(&all, "tray-stopped.png");
        assert!(ws < w1, "stopped is the glyph only");
    }

    /// The glyph is fixed: left part identical across moments; forced adds the badge; lights on keep the shape.
    #[test]
    fn glyph_fixed_forced_adds_badge_on_keeps_shape() {
        let all = frames();
        let (w, _, rh) = get(&all, "tray-rhythm@2x.png");
        let (_, _, calm) = get(&all, "tray-calm@2x.png");
        let (_, _, on) = get(&all, "tray-rhythm-on@2x.png");
        let (_, _, forced) = get(&all, "tray-rhythm-forced@2x.png");
        let (ws, _, stopped) = get(&all, "tray-stopped@2x.png");
        // glyph columns of the playing images equal the stopped image
        let rows = |b: &[u8], w: u32, cols: u32| -> Vec<u8> {
            b.chunks((w * 4) as usize)
                .flat_map(|r| r[..(cols * 4) as usize].to_vec())
                .collect()
        };
        assert_eq!(rows(&rh, w, ws), stopped);
        assert_eq!(rows(&calm, w, ws), stopped);
        assert_ne!(rh, calm);
        assert_eq!(alpha_sum(&on), alpha_sum(&rh));
        assert!(alpha_sum(&forced) > alpha_sum(&rh));
    }

    /// Bundle PNGs (written by `tauri icon`) have their nominal sizes; the SVG source ships beside them.
    #[test]
    fn app_icon_sizes_exist() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/icons");
        for (name, size) in [
            ("32x32", 32),
            ("64x64", 64),
            ("128x128", 128),
            ("128x128@2x", 256),
            ("icon", 512),
        ] {
            let bytes = std::fs::read(dir.join(format!("{name}.png"))).unwrap();
            let info = png::Decoder::new(std::io::Cursor::new(bytes.as_slice()))
                .read_info()
                .unwrap();
            let i = info.info();
            assert_eq!((i.width, i.height), (size, size), "{name}.png");
        }
        assert!(
            dir.join("app-icon.svg").is_file()
                && dir.join("icon.icns").is_file()
                && dir.join("icon.ico").is_file()
        );
    }
}
