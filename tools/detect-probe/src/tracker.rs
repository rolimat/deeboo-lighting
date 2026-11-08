//! Offline beat tracker: stands in for the MIDI clock (tempo and phase) and may look at the whole file.

const MIN_BPM: f64 = 115.0;
const MAX_BPM: f64 = 145.0;
const CLAMP_MIN_BPM: f64 = 110.0;
const CLAMP_MAX_BPM: f64 = 150.0;
const ESTIMATE_SECONDS: f64 = 600.0;
const PHASE_SECONDS: f64 = 30.0;
const SNAP_WINDOW_S: f64 = 0.070;
const HISTORY: usize = 64;

pub struct Tracked {
    pub beats: Vec<f64>,
    pub period_s: f64,
}

impl Tracked {
    #[must_use]
    pub fn bpm(&self) -> f64 {
        60.0 / self.period_s
    }

    /// True when the adapted tempo hit the edge of the allowed range (suggest `--bpm`).
    #[must_use]
    pub fn at_clamp_edge(&self) -> bool {
        (self.bpm() - CLAMP_MIN_BPM).abs() < 0.5 || (self.bpm() - CLAMP_MAX_BPM).abs() < 0.5
    }
}

/// Triangular smoothing (+-3 hops) so jittered onsets still correlate.
fn smooth(o: &[f32]) -> Vec<f64> {
    (0..o.len())
        .map(|i| {
            (0..=6usize)
                .filter_map(|k| {
                    let j = (i + k).checked_sub(3)?;
                    let w = 4.0 - (k.abs_diff(3)) as f64;
                    o.get(j).map(|&v| f64::from(v) * w)
                })
                .sum()
        })
        .collect()
}

/// Comb score of a fractional lag (in hops) over the autocorrelation table.
fn ac_at(ac: &[f64], lag: f64) -> f64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let i = lag.floor() as usize;
    let f = lag - lag.floor();
    match (ac.get(i), ac.get(i + 1)) {
        (Some(a), Some(b)) => a * (1.0 - f) + b * f,
        _ => 0.0,
    }
}

fn estimate_period(s: &[f64], hop_s: f64) -> f64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = s.len().min((ESTIMATE_SECONDS / hop_s) as usize);
    let s = &s[..n];
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let max_lag = (4.0 * 60.0 / MIN_BPM / hop_s).ceil() as usize + 2;
    let ac: Vec<f64> = (0..=max_lag)
        .map(|l| s.iter().zip(s.iter().skip(l)).map(|(a, b)| a * b).sum())
        .collect();
    let mut best = (f64::MIN, 60.0 / 128.0);
    let mut bpm = MIN_BPM;
    while bpm <= MAX_BPM {
        let p = 60.0 / bpm;
        let lag = p / hop_s;
        let score: f64 = [1.0, 2.0, 4.0].iter().map(|m| ac_at(&ac, lag * m)).sum();
        if score > best.0 {
            best = (score, p);
        }
        bpm += 0.1;
    }
    best.1
}

fn estimate_phase(s: &[f64], period_s: f64, hop_s: f64) -> f64 {
    let p = period_s / hop_s;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = s.len().min((PHASE_SECONDS / hop_s) as usize);
    let mut best = (f64::MIN, 0.0);
    let mut phi = 0.0;
    while phi < p {
        let mut score = 0.0;
        let mut x = phi;
        while x < n as f64 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                score += s.get(x.round() as usize).copied().unwrap_or(0.0);
            }
            x += p;
        }
        if score > best.0 {
            best = (score, phi);
        }
        phi += 1.0;
    }
    best.1 * hop_s
}

fn median(v: &[f32]) -> f32 {
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    s.get(s.len() / 2).copied().unwrap_or(0.0)
}

/// `onsets`: one value per analysis hop of `hop_s` seconds. `fixed_bpm`: the user gives the tempo,
/// only the phase snapping stays.
#[must_use]
pub fn track(onsets: &[f32], hop_s: f64, fixed_bpm: Option<f64>) -> Tracked {
    if onsets.is_empty() {
        return Tracked {
            beats: Vec::new(),
            period_s: 60.0 / fixed_bpm.unwrap_or(128.0),
        };
    }
    let s = smooth(onsets);
    let mut period = fixed_bpm.map_or_else(|| estimate_period(&s, hop_s), |b| 60.0 / b);
    let end = onsets.len() as f64 * hop_s;
    let mut t = estimate_phase(&s, period, hop_s);
    let mut beats = vec![t];
    let mut snapped: Vec<f32> = Vec::new();
    while t + period < end {
        let pred = t + period;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (lo, hi) = (
            ((pred - SNAP_WINDOW_S) / hop_s).ceil().max(0.0) as usize,
            ((pred + SNAP_WINDOW_S) / hop_s).floor() as usize,
        );
        let peak = (lo..=hi.min(onsets.len() - 1)).max_by(|&a, &b| onsets[a].total_cmp(&onsets[b]));
        let mut next = pred;
        if let Some(i) = peak {
            let strength = onsets[i];
            let tail = &snapped[snapped.len().saturating_sub(HISTORY)..];
            if tail.is_empty() || strength > 0.5 * median(tail) {
                next = pred + 0.5 * (i as f64 * hop_s - pred);
                snapped.push(strength);
                if fixed_bpm.is_none() {
                    period = (0.97 * period + 0.03 * (next - t))
                        .clamp(60.0 / CLAMP_MAX_BPM, 60.0 / CLAMP_MIN_BPM);
                }
            }
        }
        t = next;
        beats.push(t);
    }
    Tracked {
        beats,
        period_s: period,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP: f64 = 0.005;

    /// Impulse train at `bpm` starting at `t0`, jittered by up to +-10 ms (deterministic).
    fn train(bpm: f64, t0: f64, seconds: f64) -> Vec<f32> {
        let mut out = vec![0.0f32; (seconds / HOP) as usize];
        let period = 60.0 / bpm;
        let mut k = 0u64;
        loop {
            let jitter = ((k * 7919) % 21) as f64 - 10.0;
            let time = t0 + k as f64 * period + jitter / 1000.0;
            let idx = (time / HOP).round() as usize;
            if idx >= out.len() {
                return out;
            }
            out[idx] = 1.0;
            k += 1;
        }
    }

    #[test]
    fn tracker_locks_onto_synthetic_128_bpm() {
        let (t0, p) = (0.3, 60.0 / 128.0);
        let tracked = track(&train(128.0, t0, 40.0), HOP, None);
        assert!(
            (tracked.period_s - p).abs() / p < 0.005,
            "{}",
            tracked.bpm()
        );
        let b = tracked.beats[16];
        let off = (b - t0).rem_euclid(p);
        let off = off.min(p - off);
        assert!(off < 0.015, "phase off by {off}");
    }

    #[test]
    fn fixed_bpm_keeps_the_given_period() {
        let tracked = track(&train(128.0, 0.3, 40.0), HOP, Some(124.0));
        assert!((tracked.period_s - 60.0 / 124.0).abs() < 1e-12);
        assert!(tracked.beats.len() > 60);
    }
}
