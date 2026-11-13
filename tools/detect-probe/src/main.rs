//! Dev CLI: runs the moment detector on a recording the way the live app would
//! (blocks of audio, beats fired before the block that ends after them) and scores it against labels.

// Dev tool on audio sample counts: float/int casts are intended.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

mod decode;
mod labels;
mod live;
mod tracker;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use light_core::clock::PhraseBars;
use light_core::detect::bands::BandAnalyzer;
use light_core::detect::moment::{DetectorParams, Moment, PositionRules};
use live::{BLOCK, LiveOrder, Transition};
use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const USAGE: &str = "usage: detect-probe <audio> [--labels file.json] [--min-beats 4] [--timeline] [--bpm 128] [--realtime]
       [--features [from-to]] [--downbeat <s>]... [--phrase 4|8|16] [--rules all|none|calm,drop,riser,bars] (default: the app's rules)";
const TOLERANCE_S: f64 = 4.0;

#[derive(Debug)]
enum CliError {
    Usage(String),
    Decode(decode::DecodeError),
    Labels(labels::LabelError),
    Io(std::io::Error),
    Audio(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Usage(m) => write!(f, "{m}\n{USAGE}"),
            CliError::Decode(e) => e.fmt(f),
            CliError::Labels(e) => e.fmt(f),
            CliError::Io(e) => write!(f, "cannot read file: {e}"),
            CliError::Audio(e) => write!(f, "audio output: {e}"),
        }
    }
}

impl From<decode::DecodeError> for CliError {
    fn from(e: decode::DecodeError) -> Self {
        CliError::Decode(e)
    }
}

struct Args {
    audio: PathBuf,
    labels: Option<PathBuf>,
    min_beats: usize,
    timeline: bool,
    /// `--features`: with an optional range (s), else every beat.
    features: bool,
    features_range: Option<(f64, f64)>,
    bpm: Option<f64>,
    realtime: bool,
    downbeats: Vec<f64>,
    phrase: PhraseBars,
    rules: PositionRules,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, CliError> {
    let mut a = Args {
        audio: PathBuf::new(),
        labels: None,
        min_beats: 4,
        timeline: false,
        features: false,
        features_range: None,
        bpm: None,
        realtime: false,
        downbeats: Vec::new(),
        phrase: PhraseBars::Eight,
        rules: DetectorParams::default().rules,
    };
    let mut audio = None;
    let mut it = args.peekable();
    let bad = |what: &str| CliError::Usage(format!("bad or missing value for {what}"));
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--labels" => a.labels = Some(it.next().ok_or_else(|| bad("--labels"))?.into()),
            "--min-beats" => {
                a.min_beats = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| bad("--min-beats"))?;
            }
            "--bpm" => {
                let b: f64 = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| bad("--bpm"))?;
                if !(60.0..=200.0).contains(&b) {
                    return Err(bad("--bpm"));
                }
                a.bpm = Some(b);
            }
            "--downbeat" => {
                let d: f64 = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| bad("--downbeat"))?;
                if !d.is_finite() || d < 0.0 {
                    return Err(bad("--downbeat"));
                }
                a.downbeats.push(d);
            }
            "--phrase" => {
                a.phrase = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .and_then(PhraseBars::from_bars)
                    .ok_or_else(|| bad("--phrase"))?;
            }
            "--rules" => {
                let v = it.next().ok_or_else(|| bad("--rules"))?;
                a.rules = live::parse_rules(&v).map_err(CliError::Usage)?;
            }
            "--features" => {
                // The range is optional: only a next argument shaped like from-to is one.
                let range = if it.peek().is_some_and(|v| live::looks_like_range(v)) {
                    let v = it.next().unwrap_or_default();
                    Some(live::parse_range(&v).map_err(CliError::Usage)?)
                } else {
                    None
                };
                a.features = true;
                a.features_range = range;
            }
            "--timeline" => a.timeline = true,
            "--realtime" => a.realtime = true,
            s if s.starts_with("--") => return Err(CliError::Usage("unknown option".into())),
            _ if audio.is_none() => audio = Some(PathBuf::from(arg)),
            _ => return Err(CliError::Usage("only one audio file".into())),
        }
    }
    a.audio = audio.ok_or_else(|| CliError::Usage("missing audio file".into()))?;
    Ok(a)
}

/// Pass 1: onset function of the whole file (the tracker may look ahead; the detector may not).
fn onsets_of(path: &std::path::Path) -> Result<(Vec<f32>, u32, f64), CliError> {
    let chunks = decode::mono_chunks(path)?;
    let rate = chunks.sample_rate;
    let mut analyzer = BandAnalyzer::new(rate);
    let mut onsets = Vec::new();
    let mut frames = 0usize;
    for c in chunks {
        let c = c?;
        frames += c.len();
        analyzer.process(&c, &mut |f| onsets.push(f.onset));
    }
    Ok((onsets, rate, frames as f64 / f64::from(rate)))
}

fn run(args: &Args) -> Result<(), CliError> {
    let label_list = match &args.labels {
        Some(p) => labels::parse(&std::fs::read_to_string(p).map_err(CliError::Io)?)
            .map_err(CliError::Labels)?,
        None => Vec::new(),
    };
    // Decode errors for a missing file surface here, before any audio device is touched.
    let (onsets, rate, duration) = onsets_of(&args.audio)?;
    let hop_s = BandAnalyzer::new(rate).hop_len() as f64 / f64::from(rate);
    let tracked = tracker::track(&onsets, hop_s, args.bpm);
    drop(onsets);
    if args.bpm.is_none() && tracked.at_clamp_edge() {
        eprintln!(
            "warning: tempo estimate sits at the edge of the range ({:.1} BPM); pass --bpm",
            tracked.bpm()
        );
    }
    if args.downbeats.is_empty() {
        eprintln!("downbeat: first tracked beat");
    }
    let positions =
        live::positions(&tracked.beats, &args.downbeats, args.phrase).map_err(CliError::Usage)?;
    let params = DetectorParams {
        rules: args.rules,
        ..DetectorParams::default()
    };
    let mut live = LiveOrder::new(rate, tracked.beats.clone(), params, positions.clone());
    let mut found: Vec<Transition> = Vec::new();
    if args.realtime {
        play_realtime(&args.audio, &mut live, &mut found)?;
    } else {
        for c in decode::mono_chunks(&args.audio)? {
            live.feed(&c?, &mut |t| found.push(t));
        }
        live.finish(&mut |t| found.push(t));
    }

    let mut all = vec![Transition {
        t: tracked.beats.first().copied().unwrap_or(0.0),
        index: 0,
        moment: Moment::Rhythm,
    }];
    all.extend(&found);
    #[allow(clippy::cast_precision_loss)]
    let kick_pct = if live.beats_seen == 0 {
        0.0
    } else {
        100.0 * live.kick_beats as f64 / live.beats_seen as f64
    };
    if args.features {
        for &(t, pos, ref f, m) in &live.features {
            if args
                .features_range
                .is_none_or(|(a, b)| (a..=b).contains(&t))
            {
                println!("{}", live::feature_line(t, pos, f, m));
            }
        }
    }
    println!(
        "duration {}  beats {}  bpm {:.1}  kick {kick_pct:.0}%",
        live::mmss(duration),
        tracked.beats.len(),
        tracked.bpm()
    );
    if args.timeline {
        for l in live::timeline(&all, duration, &positions) {
            println!("{l}");
        }
    } else {
        for l in live::episodes(&all, duration, tracked.beats.len(), args.min_beats) {
            println!("{l}");
        }
    }
    if !label_list.is_empty() {
        let detected: Vec<(f64, Moment)> = found.iter().map(|t| (t.t, t.moment)).collect();
        let s = labels::score_with_beat(&detected, &label_list, TOLERANCE_S, tracked.period_s);
        for e in &s.per_episode {
            println!("{e}");
        }
        for l in &s.per_label {
            println!("label {l}");
        }
        println!(
            "score: {} hits, {} misses, {} false alarms, mean delay {:+.1} s",
            s.hits, s.misses, s.false_alarms, s.mean_delay_s
        );
    }
    Ok(())
}

/// Same loop paced to the wall clock, playing the audio on the default output. Ctrl-C just exits.
fn play_realtime(
    path: &std::path::Path,
    live: &mut LiveOrder,
    found: &mut Vec<Transition>,
) -> Result<(), CliError> {
    let chunks = decode::mono_chunks(path)?;
    let rate = chunks.sample_rate;
    let mut audio: Vec<f32> = Vec::new();
    for c in chunks {
        audio.extend(c?);
    }
    let dev = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| CliError::Audio("no default output device".into()))?;
    let cfg = dev
        .default_output_config()
        .map_err(|e| CliError::Audio(e.to_string()))?;
    let (out_rate, channels) = (cfg.sample_rate().0, usize::from(cfg.channels()));
    if cfg.sample_format() != cpal::SampleFormat::F32 {
        return Err(CliError::Audio("default output is not f32".into()));
    }
    let samples = std::sync::Arc::new(audio.clone());
    let mut pos = 0usize;
    let ratio = f64::from(rate) / f64::from(out_rate);
    let stream = dev
        .build_output_stream(
            &cfg.config(),
            move |data: &mut [f32], _| {
                for frame in data.chunks_mut(channels) {
                    // nearest-sample resampling; silence after the end
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let s = samples.get((pos as f64 * ratio) as usize).copied();
                    frame.fill(s.unwrap_or(0.0));
                    pos += 1;
                }
            },
            |e| eprintln!("audio stream error: {e}"),
            None,
        )
        .map_err(|e| CliError::Audio(e.to_string()))?;
    let start = Instant::now();
    stream.play().map_err(|e| CliError::Audio(e.to_string()))?;
    for block in audio.chunks(BLOCK) {
        let end_s = live.fed_seconds() + block.len() as f64 / f64::from(rate);
        let due = Duration::from_secs_f64(end_s);
        if let Some(wait) = due.checked_sub(start.elapsed()) {
            std::thread::sleep(wait);
        }
        live.block(block, &mut |t| {
            println!(
                "{}  {}  {}",
                live::mmss(t.t),
                t.bar_beat(),
                match t.moment {
                    Moment::Rhythm => "RHYTHM",
                    Moment::Calm => "CALM",
                    Moment::BuildUp => "BUILD-UP",
                }
            );
            found.push(t);
        });
    }
    Ok(())
}

fn main() -> ExitCode {
    let result = parse_args(std::env::args().skip(1)).and_then(|a| run(&a));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(a: &[&str]) -> Result<Args, CliError> {
        parse_args(a.iter().map(|s| (*s).to_string()))
    }

    #[test]
    fn features_does_not_eat_a_digit_audio_path() {
        let a = parse(&["--features", "3-song.mp3"]).unwrap();
        assert!(a.features && a.features_range.is_none());
        assert_eq!(a.audio, PathBuf::from("3-song.mp3"));
        let a = parse(&["--features", "160-215", "x.mp3"]).unwrap();
        assert_eq!(a.features_range, Some((160.0, 215.0)));
    }
}
