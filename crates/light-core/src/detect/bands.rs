//! Per-hop band envelopes (RMS) and kick onset.

use super::filter::{Cascade, high_band, kick_band, sub_band};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HopFrame {
    pub kick: f32,
    pub sub: f32,
    pub high: f32,
    pub onset: f32,
}

pub struct BandAnalyzer {
    kick: Cascade<4>,
    sub: Cascade<2>,
    high: Cascade<2>,
    hop: usize,
    filled: usize,
    /// Running sums of squares: kick, sub, high.
    acc: [f32; 3],
    prev_kick_log: f32,
}

impl BandAnalyzer {
    #[must_use]
    // Sample rates are far below 2^24 and hop is a small positive count.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn new(sample_rate: u32) -> BandAnalyzer {
        let fs = sample_rate as f32;
        BandAnalyzer {
            kick: kick_band(fs),
            sub: sub_band(fs),
            high: high_band(fs),
            hop: ((fs / 172.0).round() as usize).max(1),
            filled: 0,
            acc: [0.0; 3],
            prev_kick_log: 0.0,
        }
    }

    #[must_use]
    pub fn hop_len(&self) -> usize {
        self.hop
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn process(&mut self, mono: &[f32], out: &mut impl FnMut(HopFrame)) {
        for &sample in mono {
            // A driver glitch (NaN/inf/huge) must not poison the filter state; real audio is within +-1, so +-4 never clips music.
            let clean = if sample.is_finite() {
                sample.clamp(-4.0, 4.0)
            } else {
                0.0
            };
            let (k, s, h) = (
                self.kick.process(clean),
                self.sub.process(clean),
                self.high.process(clean),
            );
            self.acc[0] += k * k;
            self.acc[1] += s * s;
            self.acc[2] += h * h;
            self.filled += 1;
            if self.filled == self.hop {
                let n = self.hop as f32;
                let [kick, sub, high] = self.acc.map(|a| (a / n).sqrt());
                let kick_log = (100.0 * kick).ln_1p();
                out(HopFrame {
                    kick,
                    sub,
                    high,
                    onset: (kick_log - self.prev_kick_log).max(0.0),
                });
                self.prev_kick_log = kick_log;
                self.acc = [0.0; 3];
                self.filled = 0;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)] // test signal generation
mod tests {
    use super::*;

    fn run(chunks: &[usize], sig: &[f32], fs: u32) -> Vec<HopFrame> {
        let mut a = BandAnalyzer::new(fs);
        let mut v = Vec::new();
        let (mut pos, mut i) = (0, 0);
        while pos < sig.len() {
            let n = chunks[i % chunks.len()].min(sig.len() - pos);
            a.process(&sig[pos..pos + n], &mut |f| v.push(f));
            pos += n;
            i += 1;
        }
        v
    }

    #[test]
    fn hop_is_about_5_8_ms() {
        assert_eq!(BandAnalyzer::new(48_000).hop_len(), 279);
        assert_eq!(BandAnalyzer::new(44_100).hop_len(), 256);
    }

    #[test]
    fn emits_one_frame_per_hop_across_uneven_chunks() {
        let sig: Vec<f32> = (0..10_000_u32)
            .map(|i| ((i * 7919) % 2000) as f32 / 1000.0 - 1.0)
            .collect();
        let whole = run(&[10_000], &sig, 48_000);
        assert_eq!(whole.len(), 10_000 / 279);
        assert_eq!(run(&[1, 333, 4096], &sig, 48_000), whole);
    }

    #[test]
    fn kick_pulse_raises_onset_on_its_hop() {
        let fs = 48_000_u32;
        let mut sig = vec![0.0_f32; 9_600];
        for i in 0..1_440 {
            sig[4_800 + i] = (2.0 * std::f32::consts::PI * 60.0 * i as f32 / fs as f32).sin();
        }
        let frames = run(&[9_600], &sig, fs);
        let (peak, f) = frames
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.onset.total_cmp(&b.1.onset))
            .unwrap();
        assert!(peak.abs_diff(4_800 / 279) <= 1, "peak hop {peak}");
        assert!(f.onset > 0.5);
        let loud = frames.iter().map(|f| f.kick).fold(0.0, f32::max);
        let hi = frames.iter().map(|f| f.high).fold(0.0, f32::max);
        assert!(loud > 10.0 * hi, "kick {loud} high {hi}");
    }

    #[test]
    fn non_finite_samples_do_not_poison_filters() {
        let fs = 48_000_u32;
        let clean: Vec<f32> = (0..2 * fs)
            .map(|i| {
                let t = i as f32 / fs as f32;
                0.5 * (std::f32::consts::TAU * 60.0 * t).sin()
                    + 0.1 * (std::f32::consts::TAU * 6_000.0 * t).sin()
            })
            .collect();
        let mut dirty = clean.clone();
        dirty[20_000] = f32::NAN;
        dirty[20_001] = f32::INFINITY;
        dirty[20_002] = f32::NEG_INFINITY;
        dirty[20_003] = 3e38;
        dirty[20_004] = -3e38;
        let (a, b) = (run(&[4096], &clean, fs), run(&[4096], &dirty, fs));
        for (i, (c, d)) in a.iter().zip(&b).enumerate().skip(220) {
            // Onset is a difference of logs, so it gets an absolute tolerance.
            for (x, y) in [(c.kick, d.kick), (c.sub, d.sub), (c.high, d.high)] {
                assert!(y.is_finite(), "hop {i}");
                assert!((x - y).abs() <= 1e-3 * x.abs(), "hop {i}: {c:?} vs {d:?}");
            }
            assert!((c.onset - d.onset).abs() <= 1e-3, "hop {i}: {c:?} vs {d:?}");
        }
    }
}
