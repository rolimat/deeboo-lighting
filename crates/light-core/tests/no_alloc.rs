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
    use light_core::clock::{PhraseBars, Position};
    use light_core::detect::PositionRules;
    use light_core::detect::bands::HopFrame;
    use light_core::detect::moment::{DetectorParams, MomentDetector};
    let params = DetectorParams {
        rules: PositionRules::ALL,
        ..DetectorParams::default()
    };
    let mut d = MomentDetector::new(params, 0.0058);
    // Linear levels per beat: a groove, a break with quiet highs, a four-bar riser of highs back
    // to the groove level, then the drop on the phrase start (bar 1 of 8 at beat 96).
    let level = |beat: u32| -> (f32, f32) {
        match beat {
            0..=31 | 96.. => (0.5, 0.3),
            32..=79 => (0.0005, 0.003),
            80..=83 => (0.0005, 0.01),
            84..=87 => (0.0005, 0.03),
            88..=91 => (0.0005, 0.1),
            _ => (0.0005, 0.3),
        }
    };
    let mut seen = [false; 3];
    assert_no_alloc(|| {
        // Without a clock position.
        for beat in 0..200 {
            let (kick, high) = level(beat % 128);
            for _ in 0..90 {
                d.push(HopFrame {
                    kick,
                    sub: kick,
                    high,
                    onset: 0.0,
                });
            }
            let _ = d.on_beat(None);
        }
    });
    d.reset();
    assert_no_alloc(|| {
        // Consecutive clock positions, so riser bars close and phrase-position rules apply.
        for beat in 0..200_u32 {
            let (kick, high) = level(beat % 128);
            for _ in 0..90 {
                d.push(HopFrame {
                    kick,
                    sub: kick,
                    high,
                    onset: 0.0,
                });
            }
            let (m, _) = d.on_beat(Some(Position::at(
                u64::from(beat),
                false,
                PhraseBars::Eight,
            )));
            seen[m as usize] = true;
        }
    });
    assert!(seen.iter().all(|&s| s), "all moments reached: {seen:?}");
}
