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
    /// Near beats in a row -> Rhythm without a jump (fade-in, quieter track; after a long break the groove
    /// reference can be stale, so six bars of a steady near groove are enough).
    pub slow_return_beats: u32,
    /// Rhythm -> Build-up: kick at least this far below its reference...
    pub groove_buildup_kick_db: f32,
    /// ...and highs at least this far above theirs, for 2 beats.
    pub groove_buildup_high_db: f32,
    /// Build-up -> Calm when the 4-beat highs mean stays this far below its value at entry.
    pub collapse_db: f32,
    /// Quiet also when the 4-beat means of kick and highs are both this far below references.
    pub thin_kick_db: f32,
    pub thin_high_db: f32,
    /// No change within this many beats of the last one (except Build-up -> Rhythm).
    pub min_state_beats: u32,
    /// A full or near beat also needs the sub band within this of its reference: a drop brings the
    /// bass back, a filtered kick inside a break without bass is not the groove.
    pub sub_back_db: f32,
    /// A return to Rhythm that falls back to Calm within this many beats (a one-bar tease)...
    pub teaser_beats: u32,
    /// ...when the 4-beat mean kick deficit reaches this.
    pub teaser_kick_db: f32,
    /// Riser: the last `riser_bars` bar means (4 beats) of highs rise monotonically...
    pub riser_bars: usize,
    /// ...by at least this from first to last...
    pub riser_db: f32,
    /// ...and the last is at most this far below the highs reference.
    pub riser_level_db: f32,
    /// Approach: after this many beats of Calm, the 4-beat highs mean is at least `approach_above_db`
    /// over the highs reference and `approach_rise_db` over its value `4 * approach_bars` beats ago.
    pub approach_calm_beats: u32,
    pub approach_above_db: f32,
    pub approach_rise_db: f32,
    pub approach_bars: usize,
    /// Build-up -> Calm needs this many beats in a row below the collapse level (a riser that pauses or
    /// changes texture for a bar or two keeps the tension: only four bars well under the entry level end it)...
    pub collapse_beats: u32,
    /// ...and after a collapse there is no Calm -> Build-up for this many beats.
    pub refractory_beats: u32,
    /// A Build-up is the last bars before a drop: one that lasts this many beats without a drop was the
    /// break itself -> Calm (and the refractory starts, as for a collapse).
    pub buildup_max_beats: u32,
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
            slow_return_beats: 24,
            groove_buildup_kick_db: 6.0,
            groove_buildup_high_db: 8.0,
            collapse_db: 8.0,
            thin_kick_db: 5.0,
            thin_high_db: 8.0,
            min_state_beats: 4,
            sub_back_db: 10.0,
            teaser_beats: 16,
            teaser_kick_db: 6.0,
            riser_bars: 4,
            riser_db: 7.0,
            riser_level_db: 6.0,
            approach_calm_beats: 32,
            approach_above_db: 2.0,
            approach_rise_db: 2.0,
            approach_bars: 8,
            collapse_beats: 16,
            refractory_beats: 16,
            buildup_max_beats: 96,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatFeatures {
    pub kick_db: f32,
    pub sub_db: f32,
    pub high_db: f32,
    pub kick_ref_db: f32,
    pub sub_ref_db: f32,
    pub high_ref_db: f32,
}

/// Fixed-capacity ring of the most recent values (capacity set in `new`, never grows).
struct Ring {
    buf: Vec<f32>,
    head: usize,
    /// Explicit: `Vec::with_capacity` may reserve more than asked.
    cap: usize,
}

impl Ring {
    fn new(cap: usize) -> Ring {
        let cap = cap.max(1);
        Ring {
            buf: Vec::with_capacity(cap),
            head: 0,
            cap,
        }
    }

    fn push(&mut self, v: f32) {
        if self.buf.len() < self.cap {
            self.buf.push(v);
        } else {
            self.buf[self.head] = v;
            self.head = (self.head + 1) % self.buf.len();
        }
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    fn is_full(&self) -> bool {
        self.buf.len() == self.cap
    }

    /// `i`-th stored value, oldest first.
    fn get(&self, i: usize) -> f32 {
        self.buf[(self.head + i) % self.buf.len()]
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
/// A riser bar may sit this far under the highest bar before it.
const MONO_TOL: f32 = 0.5;

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
    sub_hist: Ring,
    high_hist: Ring,
    /// 4-beat highs means (`4 * approach_bars + 1`) and bar means of highs (`riser_bars`).
    high4_hist: Ring,
    bars: Ring,
    bar_sum: f32,
    bar_beats: u32,
    since_return: u32,
    since_collapse: u32,
    collapse_run: u32,
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
            sub_hist: Ring::new(cap),
            high_hist: Ring::new(cap),
            high4_hist: Ring::new(4 * params.approach_bars.max(1) + 1),
            bars: Ring::new(params.riser_bars),
            bar_sum: 0.0,
            bar_beats: 0,
            // The start of the track in Rhythm is not a return.
            since_return: u32::MAX,
            since_collapse: u32::MAX,
            collapse_run: 0,
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

    /// Adds this beat to the current bar of highs; when a bar completes, tells whether the last
    /// `riser_bars` bar means form a riser.
    #[allow(clippy::cast_precision_loss)]
    fn riser(&mut self, high_db: f32, high_ref_db: f32) -> bool {
        self.bar_sum += high_db;
        self.bar_beats += 1;
        if self.bar_beats < 4 {
            return false;
        }
        self.bars.push(self.bar_sum / 4.0);
        self.bar_sum = 0.0;
        self.bar_beats = 0;
        if !self.bars.is_full() {
            return false;
        }
        let (first, last) = (self.bars.get(0), self.bars.get(self.bars.len() - 1));
        let mut top = first;
        let mut mono = true;
        for i in 1..self.bars.len() {
            mono &= self.bars.get(i) >= top - MONO_TOL;
            top = top.max(self.bars.get(i));
        }
        mono && last - first >= self.p.riser_db && last >= high_ref_db - self.p.riser_level_db
    }

    /// The state after this beat. `evidence`: a riser or an approach (used only from Calm).
    fn next_moment(&mut self, evidence: bool, high4: f32) -> Moment {
        let p = self.p;
        self.in_state = self.in_state.saturating_add(1);
        self.since_return = self.since_return.saturating_add(1);
        self.since_collapse = self.since_collapse.saturating_add(1);
        let may = self.in_state >= p.min_state_beats;
        let drop = (self.jump_kick && self.full_run >= p.rhythm_after_full)
            || (self.jump_high && self.full_run >= p.hats_after_full)
            || self.near_run >= p.slow_return_beats;
        match self.moment {
            Moment::Rhythm => {
                // A one-bar tease: the return does not hold.
                let teaser = self.since_return <= p.teaser_beats
                    && self.beats >= 4
                    && self.kick_def.iter().sum::<f32>() / 4.0 <= -p.teaser_kick_db;
                if may && (self.quiet_run >= p.calm_after_quiet || teaser) {
                    Moment::Calm
                } else if may && self.groove_run >= 2 {
                    Moment::BuildUp
                } else {
                    Moment::Rhythm
                }
            }
            Moment::Calm if may && drop => Moment::Rhythm,
            Moment::Calm if may && evidence && self.since_collapse >= p.refractory_beats => {
                Moment::BuildUp
            }
            Moment::Calm => Moment::Calm,
            // The drop is exempt from `min_state_beats`.
            Moment::BuildUp if drop => Moment::Rhythm,
            Moment::BuildUp => {
                let low = high4 < self.entry_high - p.collapse_db;
                self.collapse_run = if low { self.collapse_run + 1 } else { 0 };
                // A collapse, or a Build-up that outlasted `buildup_max_beats`: Calm and the refractory.
                if (may && self.collapse_run >= p.collapse_beats)
                    || self.in_state >= p.buildup_max_beats
                {
                    self.since_collapse = 0;
                    Moment::Calm
                } else {
                    Moment::BuildUp
                }
            }
        }
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

        let (kick_ref_db, sub_ref_db, high_ref_db) = if self.kick_hist.buf.len() < REF_MIN_BEATS {
            (kick_db, sub_db, high_db)
        } else {
            (
                self.kick_hist.median(&mut self.scratch),
                self.sub_hist.median(&mut self.scratch),
                self.high_hist.median(&mut self.scratch),
            )
        };
        let p = self.p;
        let (kd, sd, hd) = (
            kick_db - kick_ref_db,
            sub_db - sub_ref_db,
            high_db - high_ref_db,
        );
        let n = self.beats; // beats before this one
        let mean = |s: &[f32]| s.iter().sum::<f32>() / s.len() as f32;

        // Quiet: kick gone, or highs gone while the kick is not full (a drop without hats is still a
        // groove while the kick pumps; a break where bass fills the kick band has a weaker kick band).
        let mut quiet = kd < -p.calm_kick_db || (hd < -p.calm_high_db && kd < -p.back_db);
        if n >= 4 {
            let k4 = (self.kick_def[1..].iter().sum::<f32>() + kd) / 4.0;
            let h4 = (self.high_def[1..].iter().sum::<f32>() + hd) / 4.0;
            quiet = quiet || (k4 <= -p.thin_kick_db && h4 <= -p.thin_high_db);
        }
        self.quiet_run = if quiet { self.quiet_run + 1 } else { 0 };

        let full = kd >= -p.back_db && hd >= -p.near_db && sd >= -p.sub_back_db;
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
            && mean(&self.high_def) >= -p.near_db
            && sd >= -p.sub_back_db;
        self.near_run = if near { self.near_run + 1 } else { 0 };

        shift(&mut self.kick_prev, kick_db);
        shift(&mut self.high_prev, high_db);
        let valid = self.beats.min(4);
        let high4 = mean(&self.high_prev[16 - valid..]);
        self.high4_hist.push(high4);

        // Build-up evidence (used only from Calm). Bars are 4-beat blocks from the first beat.
        let riser = self.riser(high_db, high_ref_db);
        let approach = self.moment == Moment::Calm
            && self.in_state >= p.approach_calm_beats
            && self.high4_hist.is_full()
            && high4 >= high_ref_db + p.approach_above_db
            && high4 - self.high4_hist.get(0) >= p.approach_rise_db;
        let groove_up = kd <= -p.groove_buildup_kick_db && hd >= p.groove_buildup_high_db;
        self.groove_run = if groove_up { self.groove_run + 1 } else { 0 };

        let next = self.next_moment(riser || approach, high4);

        if self.moment == Moment::Rhythm && kd >= -p.learn_db && hd >= -p.learn_db {
            self.kick_hist.push(kick_db);
            self.sub_hist.push(sub_db);
            self.high_hist.push(high_db);
        }
        if next != self.moment {
            if next == Moment::BuildUp {
                self.entry_high = high4;
                self.collapse_run = 0;
            }
            if next == Moment::Rhythm {
                self.since_return = 0;
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
                sub_ref_db,
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

    #[test]
    fn ring_keeps_at_most_its_capacity() {
        let mut r = Ring::new(4);
        for i in 0..10_u8 {
            r.push(f32::from(i));
        }
        assert_eq!(r.buf.len(), 4);
        let mut scratch = Vec::with_capacity(4);
        assert!((r.median(&mut scratch) - 7.5).abs() < 1e-6);
    }
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
        synth_hz(lens, kick, 80.0, sub, high_db)
    }

    /// Like `synth_gain` with the kick's pitch: at 115 Hz the kick still sits in the kick band
    /// but the sub band (below 60 Hz) barely sees it, so a bass note is the only sub energy.
    fn synth_hz(
        lens: &[usize],
        kick: impl Fn(usize) -> f32,
        kick_hz: f32,
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
                    x += kick(k) * 0.8 * (0.1 + 0.9 * (-t / 0.1).exp()) * (tau * kick_hz * t).sin();
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
            [
                f.kick_db,
                f.sub_db,
                f.high_db,
                f.kick_ref_db,
                f.sub_ref_db,
                f.high_ref_db,
            ]
            .iter()
            .all(|v| v.is_finite())
        })
    }

    fn groove(n: usize, kick: impl Fn(usize) -> bool, high: impl Fn(usize) -> f32) -> Out {
        let lens = beats(128.0, n);
        drive(&synth(&lens, kick, |_| true, high), &lens)
    }

    #[test]
    fn b2_defaults_match_the_tuning() {
        let p = DetectorParams::default();
        assert_eq!((p.history_beats, p.min_state_beats), (64, 4));
        assert_eq!((p.calm_after_quiet, p.rhythm_after_full), (2, 2));
        assert_eq!((p.hats_after_full, p.slow_return_beats), (4, 24));
        assert_eq!((p.teaser_beats, p.riser_bars), (16, 4));
        assert_eq!((p.approach_calm_beats, p.approach_bars), (32, 8));
        assert_eq!((p.collapse_beats, p.refractory_beats), (16, 16));
        assert_eq!(p.buildup_max_beats, 96);
        let dbs = [
            (p.learn_db, 16.0),
            (p.calm_kick_db, 16.0),
            (p.calm_high_db, 12.0),
            (p.back_db, 4.0),
            (p.near_db, 8.0),
            (p.sub_back_db, 10.0),
            (p.jump_db, 10.0),
            (p.teaser_kick_db, 6.0),
            (p.riser_db, 7.0),
            (p.riser_level_db, 6.0),
            (p.approach_above_db, 2.0),
            (p.approach_rise_db, 2.0),
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
    fn drop_without_hats_stays_rhythm() {
        // A drop without hats is still a groove while the kick pumps.
        let o = groove(48, |_| true, |k| if k < 32 { 0.0 } else { -20.0 });
        assert!(moments(&o).iter().all(|&m| m == Moment::Rhythm));
    }

    #[test]
    fn hats_dropping_with_the_kick_band_weaker_enter_calm() {
        // Hats gone and the kick band 10 dB down (kick gone, bass left): a break.
        let lens = beats(128.0, 48);
        let sig = synth_gain(
            &lens,
            |k| if k < 32 { 1.0 } else { 0.3 },
            |_| true,
            |k| if k < 32 { 0.0 } else { -20.0 },
        );
        let m = moments(&drive(&sig, &lens));
        assert!(m[..33].iter().all(|&m| m == Moment::Rhythm), "{m:?}");
        assert_eq!(m[33], Moment::Calm, "{m:?}");
    }

    /// Pitched kick (invisible to the sub band): kick on in `kick`, bass tone in `bass`.
    fn pitched(n: usize, kick: impl Fn(usize) -> f32, bass: impl Fn(usize) -> bool) -> Vec<Moment> {
        let lens = beats(128.0, n);
        moments(&drive(&synth_hz(&lens, kick, 115.0, bass, |_| 0.0), &lens))
    }

    #[test]
    fn kick_without_bass_does_not_end_a_break() {
        // Groove 0..32, break 32..48, the kick returns at full level at 48 without bass; the bass at 84.
        let m = pitched(
            140,
            |k| if (32..48).contains(&k) { 0.0 } else { 1.0 },
            |k| !(32..84).contains(&k),
        );
        assert_eq!(m[33], Moment::Calm, "{m:?}");
        // Without the sub check in `near`, 24 near beats would bring Rhythm back around beat 75.
        assert!(m[33..84].iter().all(|&m| m == Moment::Calm), "{m:?}");
        // The bass comes back at 84: the groove is whole again, so the slow return takes it.
        assert!(m[130..].iter().all(|&m| m == Moment::Rhythm), "{m:?}");
    }

    #[test]
    fn one_bar_teaser_returns_to_calm() {
        // Break at 32, a full bar at 48..52 (a drop), then the kick fades to 0.3.
        let lens = beats(128.0, 72);
        let sig = synth_gain(
            &lens,
            |k| match k {
                32..=47 => 0.0,
                52.. => 0.3,
                _ => 1.0,
            },
            |_| true,
            |_| 0.0,
        );
        let m = moments(&drive(&sig, &lens));
        assert_eq!(m[49], Moment::Rhythm, "{m:?}");
        assert_eq!(m[53], Moment::Rhythm, "{m:?}");
        assert!(m[54..].iter().all(|&m| m == Moment::Calm), "{m:?}");
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

    /// Hats for a break from beat 32: gone (-20 dB) until 48, then +4 dB per bar from -12,
    /// capped at 0 (the groove level), shifted by `shift` dB.
    fn riser_hats(shift: f32) -> impl Fn(usize) -> f32 {
        move |k| match k {
            0..=31 => 0.0,
            32..=47 => -20.0 + shift,
            _ => (-12.0 + 4.0 * ((k - 48) / 4) as f32).min(0.0) + shift,
        }
    }

    #[test]
    fn riser_in_a_break_enters_buildup() {
        // Four bars of monotonic rise that bring the highs back to the groove level.
        let o = groove(72, |k| k < 32, riser_hats(0.0));
        let m = moments(&o);
        assert!(m[33..59].iter().all(|&m| m == Moment::Calm), "{m:?}");
        assert!(m[59..].iter().all(|&m| m == Moment::BuildUp), "{m:?}");
    }

    #[test]
    fn riser_far_below_the_groove_is_not_a_buildup() {
        // The same rise ending 12 dB under the highs reference is not tension.
        let o = groove(72, |k| k < 32, riser_hats(-12.0));
        let m = moments(&o);
        assert!(m[33..].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    /// Break from beat 32 with hats 1 dB under the groove, stepping to +3.5 dB at `step`.
    fn late_hats(step: usize) -> impl Fn(usize) -> f32 {
        move |k| match k {
            0..=31 => 0.0,
            _ if k < step => -1.0,
            _ => 3.5,
        }
    }

    #[test]
    fn highs_above_the_groove_late_in_a_break_enter_buildup() {
        let m = moments(&groove(90, |k| k < 32, late_hats(70)));
        assert!(m[33..70].iter().all(|&m| m == Moment::Calm), "{m:?}");
        assert_eq!(m[75], Moment::BuildUp, "{m:?}");
    }

    #[test]
    fn the_same_rise_after_16_beats_of_calm_is_not_a_buildup_yet() {
        // Calm began at 33: until it is 32 beats old (beat 66) the approach rule is silent.
        let m = moments(&groove(66, |k| k < 32, late_hats(49)));
        assert!(m[33..].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    #[test]
    fn a_short_fill_inside_a_break_is_not_a_buildup() {
        // Two loud beats of hats (a fill) do not make a bar that rises 7 dB.
        let o = groove(
            72,
            |k| k < 32,
            |k| if (50..52).contains(&k) { 10.0 } else { 0.0 },
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
    fn buildup_collapses_to_calm_when_highs_stay_down_for_four_bars() {
        let lens = beats(128.0, 100);
        let sig = synth_gain(
            &lens,
            |k| if k < 40 { 1.0 } else { 0.3 },
            |_| true,
            |k| if (40..50).contains(&k) { 20.0 } else { 0.0 },
        );
        let m = moments(&drive(&sig, &lens));
        assert_eq!(m[49], Moment::BuildUp, "{m:?}");
        // The highs have been down for 16 beats (53..=68) only at 68.
        assert_eq!(m[67], Moment::BuildUp, "{m:?}");
        assert!(m[68..].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    #[test]
    fn a_one_bar_pause_keeps_the_buildup() {
        // A riser that pauses for two bars keeps the tension.
        let lens = beats(128.0, 80);
        let sig = synth_gain(
            &lens,
            |k| if k < 40 { 1.0 } else { 0.3 },
            |_| true,
            |k| {
                if (40..50).contains(&k) || k >= 58 {
                    20.0
                } else {
                    0.0
                }
            },
        );
        let m = moments(&drive(&sig, &lens));
        assert!(m[41..].iter().all(|&m| m == Moment::BuildUp), "{m:?}");
    }

    #[test]
    fn a_buildup_without_a_drop_ends_after_24_bars() {
        // Highs stay up the whole time, no drop: the break itself, not the last bars before a drop.
        let m = ducked(160, |k| if k < 40 { 0.0 } else { 10.0 });
        assert_eq!(m[41], Moment::BuildUp, "{m:?}");
        assert_eq!(m[136], Moment::BuildUp, "{m:?}");
        assert_eq!(m[137], Moment::Calm, "{m:?}");
        // Like a collapse, it starts the refractory.
        assert!(m[137..153].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    /// Kick ducked to 0.4 from beat 40, hats `hats(k)`.
    fn ducked(n: usize, hats: impl Fn(usize) -> f32) -> Vec<Moment> {
        let lens = beats(128.0, n);
        let sig = synth_gain(&lens, |k| if k < 40 { 1.0 } else { 0.4 }, |_| true, hats);
        moments(&drive(&sig, &lens))
    }

    #[test]
    fn pre_drop_gap_does_not_collapse_a_buildup() {
        // Two beats of silence in the hats right before a drop: not a collapse.
        let m = ducked(64, |k| match k {
            0..=39 => 0.0,
            50 | 51 => -30.0,
            _ => 10.0,
        });
        assert_eq!(m[41], Moment::BuildUp, "{m:?}");
        assert!(m[41..].iter().all(|&m| m == Moment::BuildUp), "{m:?}");
    }

    #[test]
    fn collapse_blocks_a_new_buildup_for_16_beats_after_a_16_beat_collapse() {
        // Collapse at 66; a riser would qualify at 79 but the refractory holds it back until 83.
        let m = ducked(100, |k| match k {
            0..=39 => 0.0,
            40..=49 => 10.0,
            50..=67 => -20.0,
            _ => (-12.0 + 4.0 * ((k - 68) / 4) as f32).min(0.0),
        });
        assert_eq!(m[49], Moment::BuildUp, "{m:?}");
        assert_eq!(m[66], Moment::Calm, "{m:?}");
        assert!(m[66..83].iter().all(|&m| m == Moment::Calm), "{m:?}");
        assert_eq!(m[83], Moment::BuildUp, "{m:?}");
    }

    #[test]
    fn a_drop_inside_the_first_beats_of_a_buildup_is_shown() {
        // Build-up from Rhythm at 41; the kick is back at full level from 42: a drop 2 beats in.
        let lens = beats(128.0, 52);
        let sig = synth_gain(
            &lens,
            |k| if (40..42).contains(&k) { 0.3 } else { 1.0 },
            |_| true,
            |k| if (40..42).contains(&k) { 10.0 } else { 0.0 },
        );
        let m = moments(&drive(&sig, &lens));
        assert_eq!(m[41], Moment::BuildUp, "{m:?}");
        assert_eq!(m[43], Moment::Rhythm, "{m:?}");
    }

    #[test]
    fn a_riser_right_after_the_cap_does_not_start_a_buildup() {
        // Up for 87 beats, the cap ends it at 137; a riser completes at 143 (and again at 147).
        let m = ducked(160, |k| match k {
            0..=39 | 128..=131 => 0.0,
            132..=135 => 3.0,
            136..=139 => 6.0,
            _ => 10.0,
        });
        assert_eq!(m[136], Moment::BuildUp, "{m:?}");
        assert_eq!(m[137], Moment::Calm, "{m:?}");
        assert!(m[137..153].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    #[test]
    fn a_long_groove_without_hats_does_not_drag_the_reference_down() {
        // Hats 20 dB down is below `learn_db`: those beats stay out of the references.
        let o = groove(140, |_| true, |k| if k < 32 { 0.0 } else { -20.0 });
        let f = o[139].1.unwrap();
        assert!(f.high_ref_db - f.high_db > 15.0, "{f:?}");
    }

    #[test]
    fn a_weak_kick_at_the_start_of_the_track_is_not_a_teaser() {
        // The track starts in Rhythm: that is not a return, so a weak kick after 10 beats stays Rhythm.
        let lens = beats(128.0, 24);
        let sig = synth_gain(&lens, |k| if k < 10 { 1.0 } else { 0.4 }, |_| true, |_| 0.0);
        let m = moments(&drive(&sig, &lens));
        assert!(m.iter().all(|&m| m == Moment::Rhythm), "{m:?}");
    }

    #[test]
    fn a_riser_with_a_dip_is_not_a_buildup() {
        // Bars at -20, -8, -14, -4 dB rise 16 dB overall, the last near the groove, but not monotonically.
        let o = groove(
            66,
            |k| k < 32,
            |k| match k {
                0..=31 => 0.0,
                32..=47 => -20.0,
                48..=51 => -8.0,
                52..=55 => -14.0,
                _ => -4.0,
            },
        );
        let m = moments(&o);
        assert!(m[33..].iter().all(|&m| m == Moment::Calm), "{m:?}");
    }

    #[test]
    fn highs_above_the_groove_but_flat_are_not_a_buildup() {
        // Highs 3.5 dB over the reference from before the break, flat for 8 bars: no approach.
        let m = moments(&groove(90, |k| k < 32, |k| if k < 30 { 0.0 } else { 3.5 }));
        assert!(m[33..].iter().all(|&m| m == Moment::Calm), "{m:?}");
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
        // Kick band 6 dB down and hats gone for 8 beats; then the hats are back (the kick rises
        // only 6 dB, not a kick jump).
        let lens = beats(128.0, 56);
        let sig = synth_gain(
            &lens,
            |k| if (32..40).contains(&k) { 0.5 } else { 1.0 },
            |_| true,
            |k| if (32..40).contains(&k) { -20.0 } else { 0.0 },
        );
        let o = drive(&sig, &lens);
        let m = moments(&o);
        assert_eq!(m[33], Moment::Calm);
        assert_eq!(m[42], Moment::Calm, "hats back for 3 beats: {m:?}");
        assert_eq!(m[43], Moment::Rhythm, "{:?}", &o[39..45]);
    }

    #[test]
    fn quieter_groove_returns_after_24_near_beats() {
        // Break, then the groove comes back 6 dB quieter: no jump, so it needs 6 bars.
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
        assert_eq!(back, Some(70), "{m:?}");
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
