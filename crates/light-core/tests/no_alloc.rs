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
