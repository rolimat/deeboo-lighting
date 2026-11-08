//! Hand labels of moments and windowed scoring of detected transitions.

use crate::live::mmss;
use light_core::detect::moment::Moment;
use serde::Deserialize;
use std::fmt;

const EPISODE_GAP_S: f64 = 120.0;
const EPISODE_MARGIN_S: f64 = 30.0;
const SHORT_CALM_BEATS: f64 = 8.0;
/// A Calm detected up to this late still counts: the cue is the lights settling, not the beat.
const CALM_TOLERANCE_S: f64 = 6.0;
/// A Build-up counts when detected up to this early, and anywhere until the next label (the drop).
const BUILDUP_EARLY_S: f64 = 8.0;

fn tolerance_for(moment: Moment, base_s: f64) -> f64 {
    if moment == Moment::Calm {
        base_s.max(CALM_TOLERANCE_S)
    } else {
        base_s
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Label {
    pub t: f64,
    pub moment: Moment,
}

#[derive(Debug)]
pub enum LabelError {
    Json(String),
    UnknownMoment(String),
}

impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LabelError::Json(e) => write!(f, "invalid labels file: {e}"),
            LabelError::UnknownMoment(m) => {
                write!(f, "unknown moment {m:?} (use calm, buildup or rhythm)")
            }
        }
    }
}

#[derive(Deserialize)]
struct Raw {
    t: f64,
    moment: String,
}

pub fn parse(json: &str) -> Result<Vec<Label>, LabelError> {
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(|e| LabelError::Json(e.to_string()))?;
    raw.into_iter()
        .map(|r| {
            let moment = match r.moment.as_str() {
                "calm" => Moment::Calm,
                "buildup" => Moment::BuildUp,
                "rhythm" => Moment::Rhythm,
                _ => return Err(LabelError::UnknownMoment(r.moment)),
            };
            Ok(Label { t: r.t, moment })
        })
        .collect()
}

#[derive(Debug, Default, PartialEq)]
pub struct Score {
    pub hits: usize,
    pub misses: usize,
    pub false_alarms: usize,
    pub mean_delay_s: f64,
    pub per_episode: Vec<String>,
    /// One line per label: time, moment, offset of the nearest detection of that moment in its
    /// episode window (`miss` when outside the tolerance, `none` when there is no such detection).
    pub per_label: Vec<String>,
}

/// Detection window of label `li` of an episode, ends included.
fn window_of(ep: &[Label], li: usize, tolerance_s: f64) -> (f64, f64) {
    let l = ep[li];
    if l.moment == Moment::BuildUp {
        let end = ep.get(li + 1).map_or(l.t + tolerance_s, |next| next.t);
        (l.t - BUILDUP_EARLY_S, end)
    } else {
        let tol = tolerance_for(l.moment, tolerance_s);
        (l.t - tol, l.t + tol)
    }
}

/// `detected`: transitions as (time, moment entered), time-ordered. A Calm of at most 8 beats
/// followed by Rhythm is not counted as a false alarm (nor is that Rhythm). `tolerance_s` applies
/// to Rhythm; Calm gets at least `CALM_TOLERANCE_S`. A Build-up label is hit by the nearest detection
/// from 8 s before it until the next label (or `tolerance_s` after it when none follows), and a
/// Build-up detection followed by Rhythm leads into the drop, so it is never a false alarm.
#[must_use]
pub fn score_with_beat(
    detected: &[(f64, Moment)],
    labels: &[Label],
    tolerance_s: f64,
    beat_s: f64,
) -> Score {
    let mut sorted = labels.to_vec();
    sorted.sort_by(|a, b| a.t.total_cmp(&b.t));
    let mut episodes: Vec<Vec<Label>> = Vec::new();
    for l in sorted {
        match episodes.last_mut() {
            Some(ep) if l.t - ep[ep.len() - 1].t < EPISODE_GAP_S => ep.push(l),
            _ => episodes.push(vec![l]),
        }
    }
    let mut total = Score::default();
    let mut delay_sum = 0.0;
    for ep in &episodes {
        let (from, to) = (
            ep[0].t - EPISODE_MARGIN_S,
            ep[ep.len() - 1].t + EPISODE_MARGIN_S,
        );
        let idx: Vec<usize> = (0..detected.len())
            .filter(|&i| detected[i].0 >= from && detected[i].0 <= to)
            .collect();
        let mut used = vec![false; detected.len()];
        let (mut hits, mut misses) = (0, 0);
        for (li, l) in ep.iter().enumerate() {
            let window = window_of(ep, li, tolerance_s);
            let best = idx
                .iter()
                .copied()
                .filter(|&i| !used[i] && detected[i].1 == l.moment)
                .filter(|&i| detected[i].0 >= window.0 && detected[i].0 <= window.1)
                .min_by(|&a, &b| {
                    (detected[a].0 - l.t)
                        .abs()
                        .total_cmp(&(detected[b].0 - l.t).abs())
                });
            let nearest = idx
                .iter()
                .copied()
                .filter(|&i| detected[i].1 == l.moment)
                .map(|i| detected[i].0 - l.t)
                .min_by(|a, b| a.abs().total_cmp(&b.abs()));
            let name = match l.moment {
                Moment::Calm => "calm",
                Moment::BuildUp => "buildup",
                Moment::Rhythm => "rhythm",
            };
            let shown = match (nearest, best) {
                (Some(d), Some(_)) => format!("{d:+.1}"),
                (Some(d), None) => format!("{d:+.1} miss"),
                (None, _) => "none".to_string(),
            };
            total
                .per_label
                .push(format!("{} {name} {shown}", mmss(l.t)));
            if let Some(i) = best {
                used[i] = true;
                hits += 1;
                delay_sum += detected[i].0 - l.t;
            } else {
                misses += 1;
            }
        }
        let mut false_alarms = 0;
        for (n, &i) in idx.iter().enumerate() {
            if used[i] {
                continue;
            }
            let short_calm = detected[i].1 == Moment::Calm
                && idx.get(n + 1).is_some_and(|&j| {
                    detected[j].1 == Moment::Rhythm
                        && detected[j].0 - detected[i].0 <= SHORT_CALM_BEATS * beat_s
                });
            let after_short_calm = detected[i].1 == Moment::Rhythm
                && n > 0
                && idx.get(n - 1).is_some_and(|&j| {
                    detected[j].1 == Moment::Calm
                        && detected[i].0 - detected[j].0 <= SHORT_CALM_BEATS * beat_s
                });
            let leads_into_drop = detected[i].1 == Moment::BuildUp
                && idx
                    .get(n + 1)
                    .is_some_and(|&j| detected[j].1 == Moment::Rhythm);
            if !short_calm && !after_short_calm && !leads_into_drop {
                false_alarms += 1;
            }
        }
        total.per_episode.push(format!(
            "{}-{}: {hits} hits, {misses} misses, {false_alarms} false alarms",
            mmss(from),
            mmss(to)
        ));
        total.hits += hits;
        total.misses += misses;
        total.false_alarms += false_alarms;
    }
    #[allow(clippy::cast_precision_loss)]
    if total.hits > 0 {
        total.mean_delay_s = delay_sum / total.hits as f64;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(d: &[(f64, Moment)], l: &[Label], tol: f64) -> Score {
        score_with_beat(d, l, tol, 60.0 / 128.0)
    }

    const CALM: Moment = Moment::Calm;
    const RHYTHM: Moment = Moment::Rhythm;
    const BUILD: Moment = Moment::BuildUp;

    #[test]
    fn labels_reject_unknown_moment() {
        assert!(matches!(
            parse(r#"[{"t": 1.0, "moment": "drop"}]"#),
            Err(LabelError::UnknownMoment(_))
        ));
        let ok = parse(r#"[{"t": 143.2, "moment": "calm"}]"#).unwrap();
        assert_eq!(
            ok,
            vec![Label {
                t: 143.2,
                moment: CALM
            }]
        );
    }

    #[test]
    fn score_counts_hits_misses_and_false_alarms() {
        let labels = [
            Label {
                t: 100.0,
                moment: CALM,
            },
            Label {
                t: 140.0,
                moment: RHYTHM,
            },
            // second episode, far away
            Label {
                t: 500.0,
                moment: BUILD,
            },
        ];
        let detected = [
            (10.0, BUILD),   // outside any window: ignored
            (101.0, CALM),   // hit, 1 s late
            (120.0, BUILD),  // leads into the Rhythm at 146: not a false alarm
            (146.0, RHYTHM), // 6 s late: miss + false alarm
            (505.0, BUILD),  // hit, 5 s > tolerance: miss + false alarm
        ];
        let s = score(&detected, &labels, 4.0);
        assert_eq!((s.hits, s.misses, s.false_alarms), (1, 2, 2));
        assert!((s.mean_delay_s - 1.0).abs() < 1e-9);
        assert_eq!(s.per_episode.len(), 2);
    }

    #[test]
    fn calm_gets_a_wider_window_than_other_moments() {
        let calm = [Label {
            t: 100.0,
            moment: CALM,
        }];
        let rhythm = [Label {
            t: 100.0,
            moment: RHYTHM,
        }];
        assert_eq!(score(&[(105.5, CALM)], &calm, 4.0).hits, 1);
        assert_eq!(score(&[(105.5, RHYTHM)], &rhythm, 4.0).hits, 0);
        assert_eq!(score(&[(106.5, CALM)], &calm, 4.0).hits, 0);
    }

    #[test]
    fn per_label_lists_the_nearest_offset_even_on_a_miss() {
        let labels = [
            Label {
                t: 100.0,
                moment: CALM,
            },
            Label {
                t: 120.0,
                moment: BUILD,
            },
        ];
        let s = score(&[(101.5, CALM), (95.0, RHYTHM)], &labels, 4.0);
        assert_eq!(s.per_label, ["01:40.0 calm +1.5", "02:00.0 buildup none"]);
    }

    #[test]
    fn short_calm_followed_by_rhythm_is_not_a_false_alarm() {
        let labels = [Label {
            t: 100.0,
            moment: BUILD,
        }];
        let detected = [(90.0, CALM), (93.0, RHYTHM), (100.0, BUILD)];
        let s = score(&detected, &labels, 4.0);
        assert_eq!((s.hits, s.false_alarms), (1, 0));
    }

    #[test]
    fn buildup_hit_anywhere_before_the_drop() {
        let labels = [
            Label {
                t: 100.0,
                moment: BUILD,
            },
            Label {
                t: 130.0,
                moment: RHYTHM,
            },
        ];
        let s = score(&[(120.0, BUILD), (130.5, RHYTHM)], &labels, 4.0);
        assert_eq!((s.hits, s.misses, s.false_alarms), (2, 0, 0));
        assert_eq!(s.per_label[0], "01:40.0 buildup +20.0");
    }

    #[test]
    fn buildup_more_than_8s_early_is_a_miss() {
        let labels = [Label {
            t: 100.0,
            moment: BUILD,
        }];
        assert_eq!(score(&[(91.0, BUILD)], &labels, 4.0).hits, 0);
        assert_eq!(score(&[(92.5, BUILD)], &labels, 4.0).hits, 1);
        // With no later label the window ends `tolerance` after the label.
        assert_eq!(score(&[(105.0, BUILD)], &labels, 4.0).hits, 0);
    }

    #[test]
    fn buildup_leading_into_rhythm_is_not_a_false_alarm() {
        let labels = [
            Label {
                t: 100.0,
                moment: CALM,
            },
            Label {
                t: 130.0,
                moment: RHYTHM,
            },
        ];
        let s = score(
            &[(101.0, CALM), (120.0, BUILD), (130.0, RHYTHM)],
            &labels,
            4.0,
        );
        assert_eq!((s.hits, s.false_alarms), (2, 0));
        // Followed by Calm instead, it is a false alarm.
        let s = score(
            &[
                (101.0, CALM),
                (120.0, BUILD),
                (125.0, CALM),
                (130.0, RHYTHM),
            ],
            &labels,
            4.0,
        );
        assert_eq!(s.false_alarms, 2);
    }
}
