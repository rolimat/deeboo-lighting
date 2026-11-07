//! The event order of the live app: audio arrives in blocks and a beat fires before the block that
//! ends after it. The detector never sees audio that is later than the beat it is asked about.

use light_core::detect::bands::BandAnalyzer;
use light_core::detect::moment::{DetectorParams, Moment, MomentDetector};

pub const BLOCK: usize = 512;

/// Index after the last beat `< block_end`, starting the search at `next`.
#[must_use]
pub fn beats_due(beats: &[f64], next: usize, block_end: f64) -> usize {
    next + beats[next.min(beats.len())..].partition_point(|&b| b < block_end)
}

/// A moment change decided at beat `index` (0-based), shown at the beat time `t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub t: f64,
    pub index: usize,
    pub moment: Moment,
}

impl Transition {
    /// 4/4 bar.beat, beat 1 being the first tracked beat.
    #[must_use]
    pub fn bar_beat(&self) -> String {
        format!("{}.{}", self.index / 4 + 1, self.index % 4 + 1)
    }
}

pub struct LiveOrder {
    analyzer: BandAnalyzer,
    detector: MomentDetector,
    back_db: f32,
    beats: Vec<f64>,
    next: usize,
    fed: usize,
    sample_rate: f64,
    moment: Moment,
    pending: Vec<f32>,
    pub beats_seen: usize,
    pub kick_beats: usize,
}

impl LiveOrder {
    #[must_use]
    pub fn new(sample_rate: u32, beats: Vec<f64>, params: DetectorParams) -> LiveOrder {
        let analyzer = BandAnalyzer::new(sample_rate);
        #[allow(clippy::cast_precision_loss)]
        let hop_s = analyzer.hop_len() as f32 / sample_rate as f32;
        LiveOrder {
            analyzer,
            detector: MomentDetector::new(params, hop_s),
            back_db: params.back_db,
            beats,
            next: 0,
            fed: 0,
            sample_rate: f64::from(sample_rate),
            moment: Moment::Rhythm,
            pending: Vec::new(),
            beats_seen: 0,
            kick_beats: 0,
        }
    }

    /// End time (s) of the audio fed so far.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn fed_seconds(&self) -> f64 {
        self.fed as f64 / self.sample_rate
    }

    /// One block: fire the beats that happen before it ends, then hand it to the analyzer.
    #[allow(clippy::cast_precision_loss)]
    pub fn block(&mut self, block: &[f32], emit: &mut impl FnMut(Transition)) {
        let end = (self.fed + block.len()) as f64 / self.sample_rate;
        let due = beats_due(&self.beats, self.next, end);
        for index in self.next..due {
            let (m, feat) = self.detector.on_beat();
            if let Some(f) = feat {
                self.beats_seen += 1;
                if f.kick_db >= f.kick_ref_db - self.back_db {
                    self.kick_beats += 1;
                }
            }
            if m != self.moment {
                self.moment = m;
                emit(Transition {
                    t: self.beats[index],
                    index,
                    moment: m,
                });
            }
        }
        self.next = due;
        let det = &mut self.detector;
        self.analyzer.process(block, &mut |f| det.push(f));
        self.fed += block.len();
    }

    /// Any chunk size in; whole `BLOCK`s out.
    pub fn feed(&mut self, samples: &[f32], emit: &mut impl FnMut(Transition)) {
        self.pending.extend_from_slice(samples);
        let whole = self.pending.len() / BLOCK * BLOCK;
        let pending = std::mem::take(&mut self.pending);
        for b in pending[..whole].chunks(BLOCK) {
            self.block(b, emit);
        }
        self.pending = pending[whole..].to_vec();
    }

    pub fn finish(&mut self, emit: &mut impl FnMut(Transition)) {
        let rest = std::mem::take(&mut self.pending);
        if !rest.is_empty() {
            self.block(&rest, emit);
        }
    }
}

#[must_use]
pub fn mmss(t: f64) -> String {
    let tenths = (t.max(0.0) * 10.0).round() as u64;
    format!("{:02}:{:04.1}", tenths / 600, (tenths % 600) as f64 / 10.0)
}

fn name(m: Moment) -> &'static str {
    match m {
        Moment::Rhythm => "RHYTHM",
        Moment::Calm => "CALM",
        Moment::BuildUp => "BUILD-UP",
    }
}

/// Every transition (Rhythm included): `mm:ss.s  bar.beat  MOMENT  (duration s)`.
#[must_use]
pub fn timeline(transitions: &[Transition], total_s: f64) -> Vec<String> {
    transitions
        .iter()
        .enumerate()
        .map(|(i, tr)| {
            let end = transitions.get(i + 1).map_or(total_s, |n| n.t);
            format!(
                "{}  {}  {}  ({:.1} s)",
                mmss(tr.t),
                tr.bar_beat(),
                name(tr.moment),
                end - tr.t
            )
        })
        .collect()
}

/// One line per non-Rhythm episode of at least `min_beats`:
/// `CALM mm:ss.s (s) → BUILD-UP mm:ss.s (s) → RHYTHM mm:ss.s`.
#[must_use]
pub fn episodes(
    transitions: &[Transition],
    total_s: f64,
    total_beats: usize,
    min_beats: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < transitions.len() {
        if transitions[i].moment == Moment::Rhythm {
            i += 1;
            continue;
        }
        let start = i;
        while i < transitions.len() && transitions[i].moment != Moment::Rhythm {
            i += 1;
        }
        let (end_t, end_idx) = transitions
            .get(i)
            .map_or((total_s, total_beats), |t| (t.t, t.index));
        if end_idx - transitions[start].index < min_beats {
            continue;
        }
        let mut parts: Vec<String> = (start..i)
            .map(|k| {
                let end = transitions.get(k + 1).map_or(end_t, |n| n.t);
                format!(
                    "{} {} ({:.1} s)",
                    name(transitions[k].moment),
                    mmss(transitions[k].t),
                    end - transitions[k].t
                )
            })
            .collect();
        if transitions.get(i).is_some() {
            parts.push(format!("RHYTHM {}", mmss(end_t)));
        }
        out.push(parts.join(" → "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmss_never_shows_sixty_seconds() {
        assert_eq!(mmss(119.96), "02:00.0");
        assert_eq!(mmss(75.25), "01:15.3");
    }

    #[test]
    fn beats_due_fires_beats_before_the_block_end() {
        let beats = [0.5, 1.0, 1.5];
        assert_eq!(beats_due(&beats, 0, 1.0), 1);
        assert_eq!(beats_due(&beats, 0, 1.0001), 2);
        assert_eq!(beats_due(&beats, 1, 1.0001), 2);
        assert_eq!(beats_due(&beats, 3, 9.0), 3);
        for end in [0.4, 0.5, 1.0, 1.5, 2.0] {
            let n = beats_due(&beats, 0, end);
            assert!(beats[..n].iter().all(|&b| b < end));
            assert!(beats[n..].iter().all(|&b| b >= end));
        }
    }

    fn tr(index: usize, moment: Moment) -> Transition {
        Transition {
            t: index as f64 * 60.0 / 128.0,
            index,
            moment,
        }
    }

    #[test]
    fn timeline_lists_every_transition_with_bar_and_duration() {
        let t = [
            tr(0, Moment::Rhythm),
            tr(32, Moment::Calm),
            tr(64, Moment::Rhythm),
        ];
        let lines = timeline(&t, 45.0);
        assert_eq!(lines[0], "00:00.0  1.1  RHYTHM  (15.0 s)");
        assert_eq!(lines[1], "00:15.0  9.1  CALM  (15.0 s)");
        assert_eq!(lines[2], "00:30.0  17.1  RHYTHM  (15.0 s)");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn episodes_skip_short_ones_and_chain_calm_to_buildup() {
        let t = [
            tr(0, Moment::Rhythm),
            tr(32, Moment::Calm),
            tr(48, Moment::BuildUp),
            tr(64, Moment::Rhythm),
            tr(70, Moment::Calm),
            tr(72, Moment::Rhythm),
        ];
        let e = episodes(&t, 60.0, 100, 4);
        assert_eq!(e.len(), 1);
        assert_eq!(
            e[0],
            "CALM 00:15.0 (7.5 s) → BUILD-UP 00:22.5 (7.5 s) → RHYTHM 00:30.0"
        );
    }

    /// A kick on every beat, then silence: the live loop reports Calm at a beat after the silence.
    #[test]
    fn live_loop_detects_calm_without_future_audio() {
        let fs = 44_100u32;
        let beat = 60.0 / 128.0;
        let n = (f64::from(fs) * 40.0) as usize;
        let audio: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / f64::from(fs);
                if t > 20.0 {
                    return 0.0;
                }
                let ph = t % beat;
                ((2.0 * std::f64::consts::PI * 80.0 * t).sin() * (-ph * 12.0).exp()) as f32
            })
            .collect();
        let beats: Vec<f64> = (0..80).map(|k| f64::from(k) * beat).collect();
        let mut live = LiveOrder::new(fs, beats, DetectorParams::default());
        let mut out = Vec::new();
        live.feed(&audio, &mut |t| out.push(t));
        live.finish(&mut |t| out.push(t));
        let calm = out.iter().find(|t| t.moment == Moment::Calm).unwrap();
        assert!(calm.t > 20.0, "calm shown at {}", calm.t);
    }
}
