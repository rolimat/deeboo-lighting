//! Per-beat features and the Rhythm / Calm / Build-up state machine.

use super::bands::HopFrame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    Rhythm,
    Calm,
    BuildUp,
}

/// Thresholds are dB relative to references (medians of the last `history_beats` groove beats
/// recorded in Rhythm). Tuned on a labelled session: see `docs/detector-tuning.md`.
#[derive(Clone, Copy, Debug)]
pub struct DetectorParams {
    pub history_beats: usize,
    /// A beat whose kick and highs are both within this of their references joins the references.
    pub learn_db: f32,
    /// Kick this far below its reference counts as gone.
    pub calm_kick_db: f32,
    /// Highs this far below their reference count as gone.
    pub calm_high_db: f32,
    /// Quiet beats in a row for Rhythm -> Calm.
    pub calm_after_quiet: u32,
    /// Full beat: kick within this of its reference...
    pub back_db: f32,
    /// ...and highs within this. Also the slow-return window (4-beat means of both).
    pub near_db: f32,
    /// Sudden return: a band at least this far above the minimum of the previous 4 beats.
    pub jump_db: f32,
    /// Full beats after a kick jump -> Rhythm (a drop).
    pub rhythm_after_full: u32,
    /// Full beats after a highs-only jump -> Rhythm (hats back for a whole bar).
    pub hats_after_full: u32,
    /// Near beats in a row -> Rhythm without a jump (fade-in, quieter track).
    pub slow_return_beats: u32,
    /// Build-up: each of 3 consecutive 4-beat highs means at least this far above the previous.
    pub buildup_rise_db: f32,
    /// A Build-up this long without a drop falls back to Calm (`u32::MAX` = never).
    pub buildup_max_beats: u32,
    /// Rhythm -> Build-up: kick at least this far below its reference...
    pub groove_buildup_kick_db: f32,
    /// ...and highs at least this far above theirs, for 2 beats.
    pub groove_buildup_high_db: f32,
    /// Build-up -> Calm when the 4-beat highs mean falls this far below its value at entry.
    pub collapse_db: f32,
    /// Quiet also when the 4-beat means of kick and highs are both this far below references.
    pub thin_kick_db: f32,
    pub thin_high_db: f32,
    /// No change within this many beats of the last one (except Build-up -> Rhythm).
    pub min_state_beats: u32,
}

impl Default for DetectorParams {
    fn default() -> Self {
        DetectorParams {
            history_beats: 64,
            learn_db: 16.0,
            calm_kick_db: 16.0,
            calm_high_db: 12.0,
            calm_after_quiet: 2,
            back_db: 4.0,
            near_db: 8.0,
            jump_db: 10.0,
            rhythm_after_full: 2,
            hats_after_full: 4,
            slow_return_beats: 32,
            buildup_rise_db: 4.0,
            buildup_max_beats: u32::MAX,
            groove_buildup_kick_db: 6.0,
            groove_buildup_high_db: 8.0,
            collapse_db: 8.0,
            thin_kick_db: 5.0,
            thin_high_db: 8.0,
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

/// Pushes `v` into a shifting window (oldest first).
fn shift<const N: usize>(w: &mut [f32; N], v: f32) {
    w.rotate_left(1);
    w[N - 1] = v;
}

pub struct MomentDetector {
    p: DetectorParams,
    moment: Moment,
    /// Sums of dB (kick, sub, high) and hop count since the last `on_beat`.
    sum: [f32; 3],
    hops: u32,
    kick_hist: Ring,
    high_hist: Ring,
    scratch: Vec<f32>,
    /// Raw kick dB of the last 4 beats and highs dB of the last 16, deficits (vs refs) of the last 4;
    /// oldest first, valid only for the last `beats` entries.
    kick_prev: [f32; 4],
    high_prev: [f32; 16],
    kick_def: [f32; 4],
    high_def: [f32; 4],
    beats: usize,
    quiet_run: u32,
    full_run: u32,
    near_run: u32,
    groove_run: u32,
    jump_kick: bool,
    jump_high: bool,
    /// 4-beat highs mean when the current Build-up began.
    entry_high: f32,
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
            kick_prev: [0.0; 4],
            high_prev: [0.0; 16],
            kick_def: [0.0; 4],
            high_def: [0.0; 4],
            beats: 0,
            quiet_run: 0,
            full_run: 0,
            near_run: 0,
            groove_run: 0,
            jump_kick: false,
            jump_high: false,
            entry_high: 0.0,
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
    #[allow(clippy::cast_precision_loss, clippy::similar_names)] // hop counts are tiny
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
        let (kd, hd) = (kick_db - kick_ref_db, high_db - high_ref_db);
        let n = self.beats; // beats before this one
        let mean = |s: &[f32]| s.iter().sum::<f32>() / s.len() as f32;

        let mut quiet = kd < -p.calm_kick_db || hd < -p.calm_high_db;
        if n >= 4 {
            let k4 = (self.kick_def[1..].iter().sum::<f32>() + kd) / 4.0;
            let h4 = (self.high_def[1..].iter().sum::<f32>() + hd) / 4.0;
            quiet = quiet || (k4 <= -p.thin_kick_db && h4 <= -p.thin_high_db);
        }
        self.quiet_run = if quiet { self.quiet_run + 1 } else { 0 };

        let full = kd >= -p.back_db && hd >= -p.near_db;
        let rise_kick = n >= 4 && kick_db - min_of(&self.kick_prev) >= p.jump_db;
        let rise_high = n >= 4 && high_db - min_of(&self.high_prev[12..]) >= p.jump_db;
        if full {
            self.full_run += 1;
            self.jump_kick |= rise_kick;
            self.jump_high |= rise_high;
        } else {
            self.full_run = 0;
            self.jump_kick = false;
            self.jump_high = false;
        }

        shift(&mut self.kick_def, kd);
        shift(&mut self.high_def, hd);
        self.beats = n.saturating_add(1);
        let near = self.beats >= 4
            && mean(&self.kick_def) >= -p.near_db
            && mean(&self.high_def) >= -p.near_db;
        self.near_run = if near { self.near_run + 1 } else { 0 };

        shift(&mut self.kick_prev, kick_db);
        shift(&mut self.high_prev, high_db);
        let up = self.beats >= 16 && {
            let m = |i: usize| mean(&self.high_prev[i..i + 4]);
            (0..3).all(|i| m(4 * i + 4) - m(4 * i) >= p.buildup_rise_db)
        };
        let groove_up = kd <= -p.groove_buildup_kick_db && hd >= p.groove_buildup_high_db;
        self.groove_run = if groove_up { self.groove_run + 1 } else { 0 };
        let valid = self.beats.min(4);
        let high4 = mean(&self.high_prev[16 - valid..]);

        self.in_state = self.in_state.saturating_add(1);
        let may = self.in_state >= p.min_state_beats;
        let drop = (self.jump_kick && self.full_run >= p.rhythm_after_full)
            || (self.jump_high && self.full_run >= p.hats_after_full)
            || self.near_run >= p.slow_return_beats;
        let next = match self.moment {
            Moment::Rhythm if may && self.quiet_run >= p.calm_after_quiet => Moment::Calm,
            Moment::Rhythm if may && self.groove_run >= 2 => Moment::BuildUp,
            Moment::Calm if may && drop => Moment::Rhythm,
            Moment::Calm if may && up => Moment::BuildUp,
            // The drop is exempt from `min_state_beats`.
            Moment::BuildUp if drop => Moment::Rhythm,
            Moment::BuildUp if may && high4 < self.entry_high - p.collapse_db => Moment::Calm,
            Moment::BuildUp if self.in_state >= p.buildup_max_beats => Moment::Calm,
            m => m,
        };

        if self.moment == Moment::Rhythm && kd >= -p.learn_db && hd >= -p.learn_db {
            self.kick_hist.push(kick_db);
            self.high_hist.push(high_db);
        }
        if next != self.moment {
            if next == Moment::BuildUp {
                self.entry_high = high4;
            }
            self.moment = next;
            self.in_state = 0;
            self.near_run = 0;
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

fn min_of(w: &[f32]) -> f32 {
    w.iter().copied().fold(f32::MAX, f32::min)
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
        synth_gain(lens, |k| if kick(k) { 1.0 } else { 0.0 }, sub, high_db)
    }

    /// Like `synth` with a linear kick gain per beat (0 = no kick).
    fn synth_gain(
        lens: &[usize],
        kick: impl Fn(usize) -> f32,
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
                if kick(k) > 0.0 {
                    x += kick(k) * 0.8 * (0.1 + 0.9 * (-t / 0.1).exp()) * (tau * 80.0 * t).sin();
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
    fn a2_defaults_match_the_tuning() {
        let p = DetectorParams::default();
        assert_eq!((p.history_beats, p.min_state_beats), (64, 4));
        assert_eq!((p.calm_after_quiet, p.rhythm_after_full), (2, 2));
        assert_eq!((p.hats_after_full, p.slow_return_beats), (4, 32));
        assert_eq!(p.buildup_max_beats, u32::MAX);
        let dbs = [
            (p.learn_db, 16.0),
            (p.calm_kick_db, 16.0),
            (p.calm_high_db, 12.0),
            (p.back_db, 4.0),
            (p.near_db, 8.0),
            (p.jump_db, 10.0),
            (p.buildup_rise_db, 4.0),
            (p.groove_buildup_kick_db, 6.0),
            (p.groove_buildup_high_db, 8.0),
            (p.collapse_db, 8.0),
            (p.thin_kick_db, 5.0),
            (p.thin_high_db, 8.0),
        ];
        for (got, want) in dbs {
            assert!((got - want).abs() < f32::EPSILON, "{got} != {want}");
        }
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
        2.0 * k.saturating_sub(40) as f32
    }

    #[test]
    fn three_rising_bars_in_a_break_enter_buildup() {
        // +2 dB/beat from beat 40: three 4-beat means each 8 dB above the previous.
        let o = groove(64, |k| k < 32, ramp);
        let m = moments(&o);
        assert_eq!(m[33], Moment::Calm);
        assert!(m[34..50].iter().all(|&m| m == Moment::Calm), "{m:?}");
        assert!(m[56..].iter().all(|&m| m == Moment::BuildUp), "{m:?}");
    }

    #[test]
    fn a_fill_bar_inside_a_break_is_not_a_buildup() {
        // One loud bar of hats (beats 48..52) is a fill, not three rising bars.
        let o = groove(
            72,
            |k| k < 32,
            |k| if (48..52).contains(&k) { 12.0 } else { 0.0 },
        );
        let m = moments(&o);
        assert!(m[33..].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    #[test]
    fn ducked_kick_with_loud_highs_enters_buildup_from_rhythm() {
        // Kick 8 dB down (still there), hats 10 dB up: a build-up inside the groove.
        let lens = beats(128.0, 64);
        let sig = synth_gain(
            &lens,
            |k| if k < 40 { 1.0 } else { 0.4 },
            |_| true,
            |k| if k < 40 { 0.0 } else { 10.0 },
        );
        let m = moments(&drive(&sig, &lens));
        assert_eq!(m[39], Moment::Rhythm);
        assert_eq!(m[41], Moment::BuildUp, "{m:?}");
        assert!(m[41..].iter().all(|&m| m == Moment::BuildUp), "{m:?}");
    }

    #[test]
    fn buildup_collapses_to_calm_when_highs_fall_without_a_drop() {
        let lens = beats(128.0, 64);
        let sig = synth_gain(
            &lens,
            |k| if k < 40 { 1.0 } else { 0.3 },
            |_| true,
            |k| if (40..50).contains(&k) { 20.0 } else { 0.0 },
        );
        let m = moments(&drive(&sig, &lens));
        assert_eq!(m[49], Moment::BuildUp, "{m:?}");
        assert!(m[54..].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    #[test]
    fn sudden_kick_return_after_risers_is_a_drop() {
        // Highs rise from 40 to 51, hold, and fall back to normal when the kick returns at 55.
        let o = groove(
            64,
            |k| !(32..55).contains(&k),
            |k| if k < 55 { ramp(k.min(51)) } else { 0.0 },
        );
        let m = moments(&o);
        assert_eq!(
            m[55],
            Moment::BuildUp,
            "one full beat is not yet a drop: {m:?}"
        );
        assert_eq!(m[56], Moment::Rhythm, "{:?}", &o[52..58]);
    }

    #[test]
    fn highs_only_return_needs_a_whole_bar() {
        let o = groove(
            56,
            |_| true,
            |k| if (32..40).contains(&k) { -20.0 } else { 0.0 },
        );
        let m = moments(&o);
        assert_eq!(m[33], Moment::Calm);
        assert_eq!(m[42], Moment::Calm, "hats back for 3 beats: {m:?}");
        assert_eq!(m[43], Moment::Rhythm, "{:?}", &o[39..45]);
    }

    #[test]
    fn quieter_groove_returns_after_32_near_beats() {
        // Break, then the groove comes back 6 dB quieter: no jump, so it needs 8 bars.
        let lens = beats(128.0, 100);
        let sig = synth_gain(
            &lens,
            |k| match k {
                0..=39 => 1.0,
                40..=43 => 0.0,
                _ => 0.5,
            },
            |_| true,
            |k| if k < 44 { 0.0 } else { -6.0 },
        );
        let m = moments(&drive(&sig, &lens));
        assert_eq!(m[41], Moment::Calm);
        let back = m[44..]
            .iter()
            .position(|&x| x == Moment::Rhythm)
            .map(|i| i + 44);
        assert_eq!(back, Some(78), "{m:?}");
    }

    #[test]
    fn thin_groove_enters_calm() {
        // Kick 6 dB and hats 10 dB down: neither is gone, but together they are thin.
        let lens = beats(128.0, 48);
        let sig = synth_gain(
            &lens,
            |k| if k < 32 { 1.0 } else { 0.5 },
            |_| true,
            |k| if k < 32 { 0.0 } else { -10.0 },
        );
        let m = moments(&drive(&sig, &lens));
        assert_eq!(m[35], Moment::Rhythm);
        assert_eq!(m[36], Moment::Calm, "{m:?}");
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
