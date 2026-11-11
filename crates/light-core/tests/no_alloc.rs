use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use light_core::detect::bands::BandAnalyzer;

#[global_allocator]
static A: AllocDisabler = AllocDisabler;

#[test]
fn process_does_not_allocate() {
    let mut analyzer = BandAnalyzer::new(48_000);
    let mut seed = 1_u32;
    let buf: Vec<f32> = (0..48_000)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            f32::from((seed >> 16) as u16) / 32_768.0 - 1.0
        })
        .collect();
    assert_no_alloc(|| analyzer.process(&buf, &mut |_| {}));
}

#[test]
fn on_beat_does_not_allocate() {
    use light_core::detect::bands::HopFrame;
    use light_core::detect::moment::{DetectorParams, MomentDetector};
    let mut d = MomentDetector::new(DetectorParams::default(), 0.0058);
    let frame = |i: u32| {
        let v = 0.01 + f32::from((i % 7) as u8) * 0.01;
        HopFrame {
            kick: v,
            sub: v,
            high: v * 0.5,
            onset: 0.0,
        }
    };
    assert_no_alloc(|| {
        for beat in 0..200 {
            for h in 0..90 {
                d.push(frame(beat + h));
            }
            let _ = d.on_beat(None);
        }
    });
}
