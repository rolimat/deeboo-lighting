//! Per-hop band envelopes (RMS) and kick onset.

use super::filter::{
    Cascade, high_band, high_mid_band, kick_band, low_mid_band, mid_band, sub_band,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HopFrame {
    pub kick: f32,
    pub sub: f32,
    pub high: f32,
    pub onset: f32,
    /// RMS of the 150-400, 400-1500 and 1500-4000 Hz bands.
    pub mids: [f32; 3],
    /// Per-hop log-RMS rise of each mid band, same formula as `onset`.
    pub mid_attack: [f32; 3],
}

pub struct BandAnalyzer {
    kick: Cascade<4>,
    sub: Cascade<2>,
    high: Cascade<2>,
    mid: [Cascade<4>; 3],
    hop: usize,
    filled: usize,
    /// Running sums of squares: kick, sub, high.
    acc: [f32; 3],
    mid_acc: [f32; 3],
    prev_kick_log: f32,
    prev_mid_log: [f32; 3],
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
            mid: [low_mid_band(fs), mid_band(fs), high_mid_band(fs)],
            hop: ((fs / 172.0).round() as usize).max(1),
            filled: 0,
            acc: [0.0; 3],
            mid_acc: [0.0; 3],
            prev_kick_log: 0.0,
            prev_mid_log: [0.0; 3],
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
            for (acc, band) in self.mid_acc.iter_mut().zip(&mut self.mid) {
                let m = band.process(clean);
                *acc += m * m;
            }
            self.filled += 1;
            if self.filled == self.hop {
                let n = self.hop as f32;
                let [kick, sub, high] = self.acc.map(|a| (a / n).sqrt());
                let mids = self.mid_acc.map(|a| (a / n).sqrt());
                let mid_log = mids.map(|m| (100.0 * m).ln_1p());
                let mut mid_attack = [0.0; 3];
                for i in 0..3 {
                    mid_attack[i] = (mid_log[i] - self.prev_mid_log[i]).max(0.0);
                }
                let kick_log = (100.0 * kick).ln_1p();
                out(HopFrame {
                    kick,
                    sub,
                    high,
                    onset: (kick_log - self.prev_kick_log).max(0.0),
                    mids,
                    mid_attack,
                });
                self.prev_kick_log = kick_log;
                self.prev_mid_log = mid_log;
                self.acc = [0.0; 3];
                self.mid_acc = [0.0; 3];
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

    #[test]
    fn mid_attacks_follow_bursts_in_their_band() {
        let fs = 48_000_u32;
        let mut sig = vec![0.0_f32; fs as usize * 2];
        for burst in 0..8 {
            for i in 0..960 {
                // Hann envelope: a hard edge would splatter into the neighbour bands.
                let env = (std::f32::consts::PI * i as f32 / 960.0).sin().powi(2);
                sig[burst * 12_000 + 2_000 + i] =
                    env * (std::f32::consts::TAU * 775.0 * i as f32 / fs as f32).sin();
            }
        }
        let frames = run(&[4096], &sig, fs);
        let peak = |b: usize| frames.iter().map(|f| f.mid_attack[b]).fold(0.0, f32::max);
        assert!(peak(1) > 1.0, "mid {}", peak(1));
        assert!(
            peak(0) < 0.5 * peak(1) && peak(2) < 0.5 * peak(1),
            "{} {} {}",
            peak(0),
            peak(1),
            peak(2)
        );
        // One clear peak per burst: count rising crossings of half the peak.
        let half = peak(1) / 2.0;
        let hits = frames
            .windows(2)
            .filter(|w| w[0].mid_attack[1] < half && w[1].mid_attack[1] >= half)
            .count();
        assert_eq!(hits, 8);
        let mut dirty = sig.clone();
        for i in (0..dirty.len()).step_by(997) {
            dirty[i] = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY][i % 3];
        }
        for f in run(&[4096], &dirty, fs) {
            for b in 0..3 {
                assert!(f.mids[b].is_finite() && f.mid_attack[b].is_finite());
                assert!(f.mid_attack[b] >= 0.0);
            }
        }
    }
}
