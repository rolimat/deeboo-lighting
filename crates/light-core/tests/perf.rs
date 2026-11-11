//! Spec §13.1 performance budget. Run with `just perf-core` (release, ignored by default).
#![allow(clippy::cast_precision_loss)] // test signal generation
use std::hint::black_box;
use std::time::Instant;

use light_core::detect::bands::BandAnalyzer;
use light_core::detect::moment::{DetectorParams, MomentDetector};

const RATE: u32 = 48_000;
const SECONDS: usize = 600;
const BEAT_LEN: usize = 22_500; // 48 kHz * 60 / 128 BPM
const CHUNK: usize = 512;

fn synth() -> Vec<f32> {
    let mut seed = 0x1234_5678_u32; // xorshift32: deterministic, no dependency
    (0..RATE as usize * SECONDS)
        .map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (f32::from((seed >> 16) as u16) / 32_768.0 - 1.0) * 0.02;
            let t = (i % BEAT_LEN) as f32 / RATE as f32;
            let kick = (std::f32::consts::TAU * 60.0 * t).sin() * (-t * 30.0).exp() * 0.8;
            kick + noise
        })
        .collect()
}

#[test]
#[ignore = "perf budget: run with `just perf-core` (release)"]
fn analyses_ten_minutes_within_budget() {
    let audio = synth();
    let mut analyzer = BandAnalyzer::new(RATE);
    let hop_seconds = analyzer.hop_len() as f32 / RATE as f32;
    let mut detector = MomentDetector::new(DetectorParams::default(), hop_seconds);
    let mut beats = 0_u32;
    let mut next_beat = BEAT_LEN;
    let mut done = 0_usize;

    let start = Instant::now();
    for chunk in audio.chunks(CHUNK) {
        analyzer.process(chunk, &mut |f| detector.push(f));
        done += chunk.len();
        while done >= next_beat {
            black_box(detector.on_beat(None));
            beats += 1;
            next_beat += BEAT_LEN;
        }
    }
    let elapsed = start.elapsed();
    black_box(&detector);

    println!("analysed {SECONDS} s of audio, {beats} beats, in {elapsed:?}");
    assert!(elapsed.as_secs_f32() < 3.0, "too slow: {elapsed:?}");
}
