//! Generates the menu bar PNG frames (18 px tall, @1x and @2x). Output is committed:
//! `cargo run -p tray-frames [dir]` (default `src-tauri/icons/tray`). Deterministic.

use std::path::Path;

const W: f32 = 24.0;
const H: f32 = 18.0;
const FRAMES: usize = 8;

type Rgb = [u8; 3];
type Shape = Box<dyn Fn(f32, f32) -> (Rgb, f32)>;

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

/// The wave's centre line for `kind` at animation `frame` (logical px).
#[allow(clippy::cast_precision_loss)] // small indices
fn wave_points(kind: &str, frame: usize) -> Vec<(f32, f32)> {
    let phase = frame as f32 / FRAMES as f32;
    (0..=96)
        .map(|i| {
            let x = 1.5 + (W - 3.0) * i as f32 / 96.0;
            let u = x / W;
            let y = match kind {
                // square wave, period 12 px
                "rhythm" => {
                    let c = (x / 12.0 + phase).fract();
                    if c < 0.5 { 5.0 } else { 13.0 }
                }
                // one slow sine across the icon
                "calm" => 8.0 - 3.0 * (std::f32::consts::TAU * (u + phase)).sin(),
                // triangle zigzag (period 6 px) growing in amplitude
                _ => {
                    let c = (x / 6.0 + phase).fract();
                    let tri = if c < 0.5 {
                        4.0 * c - 1.0
                    } else {
                        3.0 - 4.0 * c
                    };
                    8.0 - tri * (1.0 + 4.0 * u)
                }
            };
            (x, y)
        })
        .collect()
}

/// Straight-alpha RGBA, `scale` px per logical px; `cover(x, y)` gives (rgb, signed distance).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]
fn render(scale: usize, shapes: &[&dyn Fn(f32, f32) -> (Rgb, f32)]) -> Vec<u8> {
    let (w, h) = ((W as usize) * scale, (H as usize) * scale);
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
                let (rgb, d) = s(x, y);
                let sa = (0.5 - d * scale as f32).clamp(0.0, 1.0);
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

fn encode(scale: usize, rgba: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        #[allow(clippy::cast_possible_truncation)] // 48 at most
        let mut enc = png::Encoder::new(&mut buf, (24 * scale) as u32, (18 * scale) as u32);
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

fn circle(cx: f32, cy: f32, r: f32, rgb: Rgb) -> impl Fn(f32, f32) -> (Rgb, f32) {
    move |x, y| (rgb, (x - cx).hypot(y - cy) - r)
}

/// Template silhouette of the app icon: 10 LED segments around a vinyl with a hole for the label.
fn icon_shapes() -> Vec<Shape> {
    const BLACK: Rgb = [0, 0, 0];
    let (cx, cy) = (W / 2.0, H / 2.0);
    let mut out: Vec<Shape> = Vec::new();
    for i in 0..10_u8 {
        let mid = (f32::from(i) * 36.0 + 18.0).to_radians();
        let pts: Vec<_> = (-2_i8..=2)
            .map(|k| {
                let a = mid + f32::from(k) * 4.5_f32.to_radians();
                (cx + 7.4 * a.cos(), cy + 7.4 * a.sin())
            })
            .collect();
        out.push(Box::new(move |x, y| (BLACK, dist_line(&pts, x, y) - 0.8)));
    }
    // Vinyl: annulus 1.7..4.9 px (the hole is the label).
    out.push(Box::new(move |x, y| {
        (BLACK, ((x - cx).hypot(y - cy) - 3.3).abs() - 1.6)
    }));
    out
}

/// Every frame as (file name, PNG bytes).
fn frames() -> Vec<(String, Vec<u8>)> {
    const RING: Rgb = [142, 142, 147]; // mid grey: legible on light and dark menu bars
    const BLACK: Rgb = [0, 0, 0]; // template images: only alpha matters
    let mut out = Vec::new();
    for scale in [1, 2] {
        let at = if scale == 1 { "" } else { "@2x" };
        let ring = |x: f32, y: f32| (RING, ((x - 9.0).hypot(y - 9.0) - 6.2).abs() - 0.8);
        let icon = icon_shapes();
        let refs: Vec<&dyn Fn(f32, f32) -> (Rgb, f32)> = icon.iter().map(AsRef::as_ref).collect();
        out.push((
            format!("icon{at}.png"),
            encode(scale, &render(scale, &refs)),
        ));
        for (name, rgb) in [
            ("red", [255, 69, 58]),
            ("green-on", [48, 209, 88]),
            ("green-dim", [22, 100, 42]),
        ] {
            let dot = circle(19.5, 9.0, 3.0, rgb);
            out.push((
                format!("dot-{name}{at}.png"),
                encode(scale, &render(scale, &[&ring, &dot])),
            ));
        }
        for kind in ["rhythm", "calm", "buildup"] {
            for frame in 0..FRAMES {
                let pts = wave_points(kind, frame);
                let line = move |x: f32, y: f32| (BLACK, dist_line(&pts, x, y) - 0.8);
                let forced = circle(W / 2.0, 16.2, 1.3, BLACK);
                out.push((
                    format!("{kind}-{frame}{at}.png"),
                    encode(scale, &render(scale, &[&line])),
                ));
                out.push((
                    format!("{kind}-{frame}-forced{at}.png"),
                    encode(scale, &render(scale, &[&line, &forced])),
                ));
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

    #[test]
    fn tray_frames_tool_is_deterministic() {
        let a = frames();
        assert_eq!(a.len(), 2 * (3 + 3 * FRAMES * 2 + 1));
        assert_eq!(a, frames());
        assert!(a.iter().any(|(n, _)| n == "rhythm-7-forced@2x.png"));
    }

    /// Template images: every pixel is black, only alpha varies. Dot frames are coloured on purpose.
    #[test]
    fn tray_template_is_monochrome() {
        let all = frames();
        assert!(
            all.iter().any(|(n, _)| n == "icon@2x.png"),
            "app icon silhouette missing"
        );
        for (name, bytes) in all.iter().filter(|(n, _)| !n.starts_with("dot-")) {
            let mut r = png::Decoder::new(std::io::Cursor::new(bytes.as_slice()))
                .read_info()
                .unwrap();
            let mut buf = vec![0; r.output_buffer_size().unwrap()];
            r.next_frame(&mut buf).unwrap();
            assert!(
                buf.chunks(4).all(|p| p[..3] == [0, 0, 0]),
                "{name} is not black + alpha"
            );
            assert!(
                name.starts_with("icon") <= buf.chunks(4).any(|p| p[3] > 0),
                "{name} is empty"
            );
        }
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
