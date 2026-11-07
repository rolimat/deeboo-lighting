//! Per-beat features and the Rhythm / Calm / Build-up state machine.

use super::bands::HopFrame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    Rhythm,
    Calm,
    BuildUp,
}

#[derive(Clone, Copy, Debug)]
pub struct DetectorParams {
    pub history_beats: usize,
    pub calm_kick_db: f32,
    pub calm_high_db: f32,
    pub calm_after_quiet: u32,
    pub back_db: f32,
    pub rhythm_after_full: u32,
    pub buildup_rise_db: f32,
    pub drop_fall_db: f32,
    pub min_state_beats: u32,
}

impl Default for DetectorParams {
    fn default() -> Self {
        DetectorParams {
            history_beats: 64,
            calm_kick_db: 12.0,
            calm_high_db: 12.0,
            calm_after_quiet: 2,
            back_db: 4.0,
            rhythm_after_full: 2,
            buildup_rise_db: 3.0,
            drop_fall_db: 2.0,
            min_state_beats: 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatFeatures {
    pub kick_db: f32,
    pub sub_db: f32,
    pub high_db: f32,
    pub kick_ref_db: f32,
    pub high_ref_db: f32,
}

/// Fixed-capacity ring of the most recent values (capacity set in `new`, never grows).
struct Ring {
    buf: Vec<f32>,
    head: usize,
}

impl Ring {
    fn new(cap: usize) -> Ring {
        Ring {
            buf: Vec::with_capacity(cap.max(1)),
            head: 0,
        }
    }

    fn push(&mut self, v: f32) {
        if self.buf.len() < self.buf.capacity() {
            self.buf.push(v);
        } else {
            self.buf[self.head] = v;
            self.head = (self.head + 1) % self.buf.len();
        }
    }

    /// Median of the stored values; `scratch` has the same capacity as the ring.
    fn median(&self, scratch: &mut Vec<f32>) -> f32 {
        scratch.clear();
        scratch.extend_from_slice(&self.buf);
        scratch.sort_unstable_by(f32::total_cmp);
        let n = scratch.len();
        scratch[(n - 1) / 2].midpoint(scratch[n / 2])
    }
}

/// `20*log10(x + 1e-6)`, with non-finite or negative input treated as silence.
fn db(x: f32) -> f32 {
    let x = if x.is_finite() { x.max(0.0) } else { 0.0 };
    20.0 * (x + 1e-6).log10()
}

const REF_MIN_BEATS: usize = 8;

pub struct MomentDetector {
    p: DetectorParams,
    moment: Moment,
    /// Sums of dB (kick, sub, high) and hop count since the last `on_beat`.
    sum: [f32; 3],
    hops: u32,
    kick_hist: Ring,
    high_hist: Ring,
    scratch: Vec<f32>,
    /// `high_db` of the last 12 beats, oldest first.
    recent: [f32; 12],
    beats: usize,
    quiet_run: u32,
    full_run: u32,
    in_state: u32,
}

impl MomentDetector {
    /// `hop_seconds` is unused: features are running means per beat, so the hop
    /// duration does not matter (and a beat of any length is covered).
    #[must_use]
    pub fn new(params: DetectorParams, _hop_seconds: f32) -> MomentDetector {
        let cap = params.history_beats.max(1);
        MomentDetector {
            p: params,
            moment: Moment::Rhythm,
            sum: [0.0; 3],
            hops: 0,
            kick_hist: Ring::new(cap),
            high_hist: Ring::new(cap),
            scratch: Vec::with_capacity(cap),
            recent: [0.0; 12],
            beats: 0,
            quiet_run: 0,
            full_run: 0,
            in_state: params.min_state_beats,
        }
    }

    pub fn push(&mut self, frame: HopFrame) {
        self.sum[0] += db(frame.kick);
        self.sum[1] += db(frame.sub);
        self.sum[2] += db(frame.high);
        self.hops = self.hops.saturating_add(1);
    }

    #[must_use]
    pub fn moment(&self) -> Moment {
        self.moment
    }

    pub fn reset(&mut self) {
        *self = MomentDetector::new(self.p, 0.0);
    }

    /// Closes the beat that started at the previous call.
    #[allow(clippy::cast_precision_loss)] // hop counts are tiny
    pub fn on_beat(&mut self) -> (Moment, Option<BeatFeatures>) {
        if self.hops == 0 {
            return (self.moment, None);
        }
        let hops = self.hops as f32;
        let [kick_db, sub_db, high_db] = self.sum.map(|sum| sum / hops);
        self.sum = [0.0; 3];
        self.hops = 0;

        let (kick_ref_db, high_ref_db) = if self.kick_hist.buf.len() < REF_MIN_BEATS {
            (kick_db, high_db)
        } else {
            (
                self.kick_hist.median(&mut self.scratch),
                self.high_hist.median(&mut self.scratch),
            )
        };
        let p = self.p;
        let quiet =
            kick_db < kick_ref_db - p.calm_kick_db || high_db < high_ref_db - p.calm_high_db;
        let kick_back = kick_db >= kick_ref_db - p.back_db;
        let full = kick_back && high_db >= high_ref_db - p.calm_high_db;
        self.quiet_run = if quiet { self.quiet_run + 1 } else { 0 };
        self.full_run = if full { self.full_run + 1 } else { 0 };

        self.recent.rotate_left(1);
        self.recent[11] = high_db;
        self.beats = self.beats.saturating_add(1);
        let mean = |s: &[f32]| s.iter().sum::<f32>() / s.len() as f32;
        let rise = if self.beats >= 12 {
            mean(&self.recent[8..]) - mean(&self.recent[..8])
        } else {
            0.0
        };
        let max8 = self.recent[4..].iter().copied().fold(f32::MIN, f32::max);

        self.in_state = self.in_state.saturating_add(1);
        let may = self.in_state >= p.min_state_beats;
        let next = match self.moment {
            Moment::Rhythm if may && self.quiet_run >= p.calm_after_quiet => Moment::Calm,
            Moment::Calm if may && rise >= p.buildup_rise_db => Moment::BuildUp,
            Moment::Calm if may && self.full_run >= p.rhythm_after_full => Moment::Rhythm,
            // The drop is exempt from `min_state_beats`.
            Moment::BuildUp if kick_back && high_db < max8 - p.drop_fall_db => Moment::Rhythm,
            m => m,
        };

        if self.moment == Moment::Rhythm && !quiet {
            self.kick_hist.push(kick_db);
            self.high_hist.push(high_db);
        }
        if next != self.moment {
            self.moment = next;
            self.in_state = 0;
        }
        (
            self.moment,
            Some(BeatFeatures {
                kick_db,
                sub_db,
                high_db,
                kick_ref_db,
                high_ref_db,
            }),
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // test signal generation
mod tests {
    use super::*;
    use crate::detect::bands::BandAnalyzer;

    const FS: u32 = 48_000;
    type Out = Vec<(Moment, Option<BeatFeatures>)>;

    /// Kick: 80 Hz punch over a sustained floor. Sub: steady 25 Hz. Hats: noise, 2 per beat.
    /// `high_db(k)` scales the hats of beat `k`.
    fn synth(
        lens: &[usize],
        kick: impl Fn(usize) -> bool,
        sub: impl Fn(usize) -> bool,
        high_db: impl Fn(usize) -> f32,
    ) -> Vec<f32> {
        let (fs, tau) = (FS as f32, std::f32::consts::TAU);
        let (mut seed, mut g, mut out) = (1_u32, 0_usize, Vec::new());
        for (k, &len) in lens.iter().enumerate() {
            let hats = 10_f32.powf(high_db(k) / 20.0);
            for i in 0..len {
                let t = i as f32 / fs;
                let th = (i % (len / 2)) as f32 / fs;
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = f32::from((seed >> 16) as u16) / 32_768.0 - 1.0;
                let mut x = 0.3 * hats * (0.1 + 0.9 * (-th / 0.04).exp()) * noise;
                if kick(k) {
                    x += 0.8 * (0.1 + 0.9 * (-t / 0.1).exp()) * (tau * 80.0 * t).sin();
                }
                if sub(k) {
                    x += 0.05 * (tau * 25.0 * g as f32 / fs).sin();
                }
                g += 1;
                out.push(x);
            }
        }
        out
    }

    fn beats(bpm: f32, n: usize) -> Vec<usize> {
        vec![(60.0 / bpm * FS as f32).round() as usize; n]
    }

    /// One result per beat: the call that closes beat `k` is `out[k]`.
    fn drive(sig: &[f32], lens: &[usize]) -> Out {
        let mut a = BandAnalyzer::new(FS);
        let mut d = MomentDetector::new(DetectorParams::default(), a.hop_len() as f32 / FS as f32);
        let mut pos = 0;
        lens.iter()
            .map(|&len| {
                a.process(&sig[pos..pos + len], &mut |f| d.push(f));
                pos += len;
                d.on_beat()
            })
            .collect()
    }

    fn moments(o: &Out) -> Vec<Moment> {
        o.iter().map(|r| r.0).collect()
    }

    fn all_finite(o: &Out) -> bool {
        o.iter().filter_map(|r| r.1).all(|f| {
            [f.kick_db, f.sub_db, f.high_db, f.kick_ref_db, f.high_ref_db]
                .iter()
                .all(|v| v.is_finite())
        })
    }

    fn groove(n: usize, kick: impl Fn(usize) -> bool, high: impl Fn(usize) -> f32) -> Out {
        let lens = beats(128.0, n);
        drive(&synth(&lens, kick, |_| true, high), &lens)
    }

    #[test]
    fn steady_four_on_the_floor_stays_rhythm() {
        let o = groove(64, |_| true, |_| 0.0);
        assert!(moments(&o).iter().all(|&m| m == Moment::Rhythm));
        assert!(o.iter().all(|r| r.1.is_some()));
    }

    #[test]
    fn two_missing_kicks_enter_calm_on_second_beat() {
        let o = groove(48, |k| k < 32, |_| 0.0);
        let m = moments(&o);
        assert!(m[..33].iter().all(|&m| m == Moment::Rhythm), "{m:?}");
        assert_eq!(m[33], Moment::Calm, "{:?}", &o[30..36]);
    }

    #[test]
    fn hats_dropping_with_bass_kept_enter_calm() {
        let o = groove(48, |_| true, |k| if k < 32 { 0.0 } else { -20.0 });
        let m = moments(&o);
        assert!(m[..33].iter().all(|&m| m == Moment::Rhythm), "{m:?}");
        assert_eq!(m[33], Moment::Calm, "{:?}", &o[30..36]);
    }

    #[test]
    fn full_beats_return_to_rhythm_after_two() {
        let o = groove(48, |k| !(32..36).contains(&k), |_| 0.0);
        let m = moments(&o);
        assert_eq!(m[33], Moment::Calm);
        assert_eq!(m[36], Moment::Calm, "one full beat is not enough");
        assert_eq!(m[37], Moment::Rhythm, "{:?}", &o[34..40]);
    }

    fn ramp(k: usize) -> f32 {
        k.saturating_sub(40) as f32
    }

    #[test]
    fn rising_highs_enter_buildup() {
        let o = groove(56, |k| k < 32, ramp);
        let m = moments(&o);
        assert_eq!(m[33], Moment::Calm);
        assert!(m[34..40].iter().all(|&m| m == Moment::Calm), "{m:?}");
        assert_eq!(m[48], Moment::BuildUp, "{:?}", &o[40..50]);
    }

    #[test]
    fn buildup_with_kick_and_rising_highs_stays_buildup() {
        let o = groove(64, |k| !(32..50).contains(&k), ramp);
        let m = moments(&o);
        assert_eq!(m[48], Moment::BuildUp);
        assert!(m[48..].iter().all(|&m| m == Moment::BuildUp), "{m:?}");
    }

    #[test]
    fn kick_back_after_risers_stop_is_drop() {
        // Highs rise from 40 to 51, hold, and fall back to normal when the kick returns at 55.
        let o = groove(
            64,
            |k| !(32..55).contains(&k),
            |k| if k < 55 { ramp(k.min(51)) } else { 0.0 },
        );
        let m = moments(&o);
        assert_eq!(m[54], Moment::BuildUp, "{m:?}");
        assert_eq!(m[55], Moment::Rhythm, "{:?}", &o[52..58]);
    }

    #[test]
    fn short_gap_flips_at_most_once() {
        let o = groove(64, |k| !(32..36).contains(&k), |_| 0.0);
        let m = moments(&o);
        let flips = m.windows(2).filter(|w| w[0] != w[1]).count();
        assert_eq!(flips, 2, "{m:?}");
        assert_eq!(m[63], Moment::Rhythm);
    }

    fn with_silence_after(n_groove: usize, n_total: usize) -> Out {
        let lens = beats(128.0, n_total);
        let mut sig = synth(&lens, |_| true, |_| true, |_| 0.0);
        let cut: usize = lens[..n_groove].iter().sum();
        sig[cut..].fill(0.0);
        drive(&sig, &lens)
    }

    #[test]
    fn silence_yields_calm_without_nan() {
        let o = with_silence_after(32, 48);
        assert!(all_finite(&o));
        let m = moments(&o);
        assert_eq!(m[32], Moment::Rhythm);
        assert_eq!(m[33], Moment::Calm, "{:?}", &o[30..36]);
    }

    #[test]
    fn cold_silence_stays_finite() {
        let lens = beats(128.0, 16);
        let o = drive(&vec![0.0; lens.iter().sum()], &lens);
        assert!(all_finite(&o));
    }

    #[test]
    fn clipped_input_stays_finite() {
        let lens = beats(128.0, 32);
        let sig: Vec<f32> = synth(&lens, |_| true, |_| true, |_| 0.0)
            .iter()
            .map(|x| (x * 100.0).clamp(-1.0, 1.0))
            .collect();
        assert!(all_finite(&drive(&sig, &lens)));
    }

    #[test]
    fn features_follow_variable_beat_interval() {
        let lens: Vec<usize> = (0..40)
            .map(|i| if i % 2 == 0 { 22_080 } else { 23_040 }) // 0.46 s / 0.48 s
            .collect();
        let o = drive(&synth(&lens, |_| true, |_| true, |_| 0.0), &lens);
        assert!(moments(&o).iter().all(|&m| m == Moment::Rhythm));
        let k: Vec<f32> = o[8..].iter().map(|r| r.1.unwrap().kick_db).collect();
        let (lo, hi) = k
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(hi - lo < 1.5, "kick_db spread {lo}..{hi}");
    }

    #[test]
    fn beat_without_frames_keeps_moment() {
        let o = with_silence_after(32, 36);
        assert_eq!(o[35].0, Moment::Calm);
        let mut d = MomentDetector::new(DetectorParams::default(), 0.0058);
        assert_eq!(d.on_beat(), (Moment::Rhythm, None));
        d.moment = Moment::Calm;
        assert_eq!(d.on_beat(), (Moment::Calm, None));
        assert_eq!(d.moment(), Moment::Calm);
    }
}
