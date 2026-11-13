//! The event order of the live app: audio arrives in blocks and a beat fires before the block that
//! ends after it. The detector never sees audio that is later than the beat it is asked about.

use light_core::clock::{PhraseBars, Position};
use light_core::detect::BeatFeatures;
use light_core::detect::bands::BandAnalyzer;
use light_core::detect::moment::{DetectorParams, Moment, MomentDetector, PositionRules};

pub const BLOCK: usize = 512;

/// Index after the last beat `< block_end`, starting the search at `next`.
#[must_use]
pub fn beats_due(beats: &[f64], next: usize, block_end: f64) -> usize {
    next + beats[next.min(beats.len())..].partition_point(|&b| b < block_end)
}

/// Clock position of every tracked beat: each anchor (seconds) snaps to the nearest tracked beat,
/// which becomes phrase bar 1 beat 1. No anchor means the first beat. Like a Start in the app, a
/// re-anchor leaves the beat just before it without a position. An anchor past the last beat is an error.
pub fn positions(
    beats: &[f64],
    anchors: &[f64],
    phrase: PhraseBars,
) -> Result<Vec<Option<Position>>, String> {
    let Some(&last) = beats.last() else {
        return Ok(Vec::new());
    };
    if let Some(a) = anchors.iter().find(|&&a| a > last) {
        return Err(format!(
            "--downbeat {a} is past the last tracked beat ({last:.1} s)"
        ));
    }
    let first = [beats[0]];
    let anchors = if anchors.is_empty() { &first } else { anchors };
    let mut snapped: Vec<usize> = anchors
        .iter()
        .filter_map(|&a| {
            (0..beats.len()).min_by(|&i, &j| (beats[i] - a).abs().total_cmp(&(beats[j] - a).abs()))
        })
        .collect();
    snapped.sort_unstable();
    snapped.dedup();
    Ok((0..beats.len())
        .map(|i| {
            if snapped.iter().skip(1).any(|&s| s == i + 1) {
                return None;
            }
            let k = snapped.partition_point(|&s| s <= i);
            let start = *snapped.get(k.checked_sub(1)?)?;
            Some(Position::at((i - start) as u64, false, phrase))
        })
        .collect())
}

/// True for `^\d+(\.\d+)?-\d+(\.\d+)?$`: what `--features` accepts as its optional range.
#[must_use]
pub fn looks_like_range(s: &str) -> bool {
    let num = |n: &str| {
        let (i, f) = n.split_once('.').map_or((n, None), |(i, f)| (i, Some(f)));
        let digits = |d: &str| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit());
        digits(i) && f.is_none_or(digits)
    };
    s.split_once('-').is_some_and(|(a, b)| num(a) && num(b))
}

/// all, none or a comma list of calm,drop,riser,bars.
pub fn parse_rules(s: &str) -> Result<PositionRules, String> {
    match s {
        "all" => return Ok(PositionRules::ALL),
        "none" => return Ok(PositionRules::NONE),
        _ => {}
    }
    let mut r = PositionRules::NONE;
    for w in s.split(',') {
        match w {
            "calm" => r.phrase_quick = true,
            "drop" => r.bar_drop = true,
            "riser" => r.late_riser = true,
            "bars" => r.aligned_bars = true,
            _ => {
                return Err(format!(
                    "unknown rule '{w}' (all, none, calm, drop, riser, bars)"
                ));
            }
        }
    }
    Ok(r)
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

/// One closed beat: its time, clock position, features and the moment after it.
pub type FeatureRow = (f64, Option<Position>, BeatFeatures, Moment);

pub struct LiveOrder {
    analyzer: BandAnalyzer,
    detector: MomentDetector,
    back_db: f32,
    beats: Vec<f64>,
    positions: Vec<Option<Position>>,
    next: usize,
    fed: usize,
    sample_rate: f64,
    moment: Moment,
    pending: Vec<f32>,
    pub beats_seen: usize,
    pub kick_beats: usize,
    pub features: Vec<FeatureRow>,
}

impl LiveOrder {
    #[must_use]
    pub fn new(
        sample_rate: u32,
        beats: Vec<f64>,
        params: DetectorParams,
        positions: Vec<Option<Position>>,
    ) -> LiveOrder {
        let analyzer = BandAnalyzer::new(sample_rate);
        #[allow(clippy::cast_precision_loss)]
        let hop_s = analyzer.hop_len() as f32 / sample_rate as f32;
        LiveOrder {
            analyzer,
            detector: MomentDetector::new(params, hop_s),
            back_db: params.back_db,
            beats,
            positions,
            next: 0,
            fed: 0,
            sample_rate: f64::from(sample_rate),
            moment: Moment::Rhythm,
            pending: Vec::new(),
            beats_seen: 0,
            kick_beats: 0,
            features: Vec::new(),
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
            // The call closes beat `index - 1`.
            let closed = index
                .checked_sub(1)
                .and_then(|i| self.positions.get(i).copied().flatten());
            let closed_t = index.checked_sub(1).map(|i| self.beats[i]);
            let (m, feat) = self.detector.on_beat(closed);
            if let (Some(f), Some(t)) = (feat, closed_t) {
                self.features.push((t, closed, f, m));
            }
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

/// `P<phrase>.<bar>.<beat>`.
fn phrase_label(p: Position) -> String {
    format!(
        "P{}.{}.{}",
        p.beat / (4 * u64::from(p.phrase_bars)) + 1,
        p.bar_in_phrase,
        p.beat_in_bar
    )
}

/// `mm:ss.s  P1.2.3|-  kick dB (vs ref)  sub ..  high ..  MOMENT`.
#[must_use]
pub fn feature_line(t: f64, pos: Option<Position>, f: &BeatFeatures, m: Moment) -> String {
    let band = |n: &str, db: f32, r: f32| format!("{n} {db:.1} ({:+.1})", db - r);
    format!(
        "{}  {}  {}  {}  {}  {}",
        mmss(t),
        pos.map_or("-".to_string(), phrase_label),
        band("kick", f.kick_db, f.kick_ref_db),
        band("sub", f.sub_db, f.sub_ref_db),
        band("high", f.high_db, f.high_ref_db),
        name(m)
    )
}

/// `from-to` in seconds, `from < to`.
pub fn parse_range(s: &str) -> Result<(f64, f64), String> {
    let bad = || format!("bad range '{s}' (use from-to seconds, e.g. 160-215)");
    let (a, b) = s.split_once('-').ok_or_else(bad)?;
    let (a, b): (f64, f64) = (a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?);
    if a.is_finite() && b.is_finite() && 0.0 <= a && a < b {
        Ok((a, b))
    } else {
        Err(bad())
    }
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
pub fn timeline(
    transitions: &[Transition],
    total_s: f64,
    positions: &[Option<Position>],
) -> Vec<String> {
    transitions
        .iter()
        .enumerate()
        .map(|(i, tr)| {
            let end = transitions.get(i + 1).map_or(total_s, |n| n.t);
            // The closed beat's position, as `--features` shows it.
            let phrase = tr
                .index
                .checked_sub(1)
                .and_then(|c| positions.get(c))
                .copied()
                .flatten()
                .map_or(String::new(), |p| format!("  {}", phrase_label(p)));
            format!(
                "{}  {}{phrase}  {}  ({:.1} s)",
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
    fn positions_count_from_the_snapped_anchor() {
        let beats: Vec<f64> = (0..12).map(|k| f64::from(k) * 0.5).collect();
        let p = positions(&beats, &[2.1], PhraseBars::Eight).unwrap();
        assert_eq!(p.len(), 12);
        assert_eq!(p[3], None);
        assert_eq!(p[4], Some(Position::at(0, false, PhraseBars::Eight)));
        assert_eq!(p[9], Some(Position::at(5, false, PhraseBars::Eight)));
    }

    #[test]
    fn second_anchor_restarts_the_count() {
        let beats: Vec<f64> = (0..12).map(|k| f64::from(k) * 0.5).collect();
        let p = positions(&beats, &[1.0, 4.0], PhraseBars::Four).unwrap();
        assert_eq!(p[0], None);
        assert_eq!(p[2], Some(Position::at(0, false, PhraseBars::Four)));
        assert_eq!(p[6], Some(Position::at(4, false, PhraseBars::Four)));
        assert_eq!(p[8], Some(Position::at(0, false, PhraseBars::Four)));
        assert_eq!(p[11], Some(Position::at(3, false, PhraseBars::Four)));
    }

    #[test]
    fn rules_flag_parses() {
        assert_eq!(parse_rules("all"), Ok(PositionRules::ALL));
        assert_eq!(parse_rules("none"), Ok(PositionRules::NONE));
        let r = parse_rules("calm,riser").unwrap();
        assert!(r.phrase_quick && r.late_riser && !r.bar_drop && !r.aligned_bars);
        assert!(parse_rules("calm,nope").is_err());
    }

    #[test]
    fn timeline_shows_the_phrase_position_when_known() {
        let beats: Vec<f64> = (0..40).map(|k| f64::from(k) * 0.5).collect();
        let pos = positions(&beats, &[2.0], PhraseBars::Four).unwrap();
        let lines = timeline(&[tr(0, Moment::Rhythm), tr(6, Moment::Calm)], 20.0, &pos);
        assert!(!lines[0].contains('P'), "{}", lines[0]);
        assert!(lines[1].contains("P1.1.2"), "{}", lines[1]);
    }

    #[test]
    fn timeline_uses_the_closed_beat() {
        let beats: Vec<f64> = (0..40).map(|k| f64::from(k) * 0.5).collect();
        let mut pos = positions(&beats, &[2.0], PhraseBars::Four).unwrap();
        pos[5] = None; // the closed beat has none, the transition beat does
        let lines = timeline(&[tr(6, Moment::Calm)], 20.0, &pos);
        assert!(!lines[0].contains('P'), "{}", lines[0]);
    }

    #[test]
    fn no_anchor_starts_at_first_beat() {
        let beats: Vec<f64> = (0..8).map(|k| 1.0 + f64::from(k) * 0.5).collect();
        let p = positions(&beats, &[], PhraseBars::Four).unwrap();
        assert_eq!(p[0], Some(Position::at(0, false, PhraseBars::Four)));
        assert_eq!(p[3], Some(Position::at(3, false, PhraseBars::Four)));
    }

    #[test]
    fn anchor_past_end_is_an_error() {
        let beats: Vec<f64> = (0..8).map(|k| f64::from(k) * 0.5).collect();
        assert!(positions(&beats, &[99.0], PhraseBars::Four).is_err());
        assert!(positions(&beats, &[3.5], PhraseBars::Four).is_ok());
    }

    #[test]
    fn beat_before_second_anchor_has_no_position() {
        let beats: Vec<f64> = (0..12).map(|k| f64::from(k) * 0.5).collect();
        let p = positions(&beats, &[1.0, 4.0], PhraseBars::Four).unwrap();
        assert_eq!(p[7], None);
        assert_eq!(p[8], Some(Position::at(0, false, PhraseBars::Four)));
        assert!(p[6].is_some() && p[1].is_none());
    }

    #[test]
    fn range_shape_is_strict() {
        assert!(looks_like_range("160-215") && looks_like_range("1.5-20.25"));
        for s in ["160", "song-3.mp3", "1-", "-5", "1.-2", "1-2-3", "3-4.mp3"] {
            assert!(!looks_like_range(s), "{s}");
        }
    }

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
        let lines = timeline(&t, 45.0, &[]);
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
        let mut live = LiveOrder::new(fs, beats, DetectorParams::default(), Vec::new());
        let mut out = Vec::new();
        live.feed(&audio, &mut |t| out.push(t));
        live.finish(&mut |t| out.push(t));
        let calm = out.iter().find(|t| t.moment == Moment::Calm).unwrap();
        assert!(calm.t > 20.0, "calm shown at {}", calm.t);
    }

    #[test]
    fn live_order_hands_the_closed_beats_position_to_on_beat() {
        let fs = 8_000u32;
        let beats: Vec<f64> = (0..12).map(|k| 0.5 + f64::from(k) * 0.5).collect();
        let pos = positions(&beats, &[], PhraseBars::Four).unwrap();
        let mut live = LiveOrder::new(fs, beats.clone(), DetectorParams::default(), pos.clone());
        live.feed(&vec![0.1f32; fs as usize * 8], &mut |_| {});
        assert!(live.features.len() > 4);
        for (t, p, _, _) in &live.features {
            let i = beats.iter().position(|b| (b - t).abs() < 1e-9).unwrap();
            assert_eq!(*p, pos[i]);
        }
    }

    #[test]
    fn feature_line_formats_values_and_position() {
        let f = BeatFeatures {
            kick_db: -12.34,
            sub_db: -20.0,
            high_db: -31.26,
            kick_ref_db: -10.0,
            sub_ref_db: -20.0,
            high_ref_db: -30.0,
        };
        let beats: Vec<f64> = (0..40).map(|k| f64::from(k) * 0.5).collect();
        let pos = positions(&beats, &[2.0], PhraseBars::Four).unwrap();
        assert_eq!(
            feature_line(75.25, pos[6], &f, Moment::BuildUp),
            "01:15.3  P1.1.3  kick -12.3 (-2.3)  sub -20.0 (+0.0)  high -31.3 (-1.3)  BUILD-UP"
        );
        assert!(feature_line(0.0, None, &f, Moment::Rhythm).contains("  -  kick"));
    }

    #[test]
    fn parse_range_accepts_from_to() {
        assert_eq!(parse_range("160-215"), Ok((160.0, 215.0)));
        assert!(parse_range("215-160").is_err());
        assert!(parse_range("x").is_err());
    }
}
