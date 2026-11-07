//! Hand labels of moments and windowed scoring of detected transitions.

use crate::live::mmss;
use light_core::detect::moment::Moment;
use serde::Deserialize;
use std::fmt;

const EPISODE_GAP_S: f64 = 120.0;
const EPISODE_MARGIN_S: f64 = 30.0;
const SHORT_CALM_BEATS: f64 = 8.0;

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
}

/// `detected`: transitions as (time, moment entered), time-ordered. A Calm of at most 8 beats
/// followed by Rhythm is not counted as a false alarm (nor is that Rhythm).
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
        for l in ep {
            let best = idx
                .iter()
                .copied()
                .filter(|&i| !used[i] && detected[i].1 == l.moment)
                .filter(|&i| (detected[i].0 - l.t).abs() <= tolerance_s)
                .min_by(|&a, &b| {
                    (detected[a].0 - l.t)
                        .abs()
                        .total_cmp(&(detected[b].0 - l.t).abs())
                });
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
            if !short_calm && !after_short_calm {
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
            (120.0, BUILD),  // false alarm
            (146.0, RHYTHM), // 6 s late: miss + false alarm
            (505.0, BUILD),  // hit, 5 s > tolerance: miss + false alarm
        ];
        let s = score(&detected, &labels, 4.0);
        assert_eq!((s.hits, s.misses, s.false_alarms), (1, 2, 3));
        assert!((s.mean_delay_s - 1.0).abs() < 1e-9);
        assert_eq!(s.per_episode.len(), 2);
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
}
