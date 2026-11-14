#![allow(clippy::expect_used)] // helpers are outside allow-expect-in-tests
//! The editor's simulation (`ui/src/editor/sequences/player.ts`) must pick the same step as the
//! engine for the same half-beat count. The fixture is what the real engine shows; regenerate it with
//! `UPDATE_FIXTURES=1 cargo test -p light-core --test player_fixture`.
use light_core::clock::PhraseBars;
use light_core::detect::Moment;
use light_core::engine::{Engine, EngineConfig, UiCommand};
use light_core::fade;
use light_core::library::{Library, SeqRef};
use light_core::model::{Colour, Role, Sequence, Step, StepLen};
use light_core::paint::PaintCommand;
use light_core::scene::apply;

const SEGMENTS: u8 = 1;
const HALF_BEATS: u64 = 64;
const T0: u64 = 1_000_000;
const TICK_US: u64 = 41_666; // ~60 BPM: 500 ms per half-beat
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../ui/src/editor/sequences/player-fixture.json"
);

use StepLen::{Four, Half, One, Two};

/// Step `i` is painted with role `ROLES[i]` on one segment: one message per step (the scheduler drops
/// the tail of a longer scene when the next half-beat plans over it), and the colour tells the step.
const ROLES: [Role; 5] = [
    Role::Principal,
    Role::Contraste,
    Role::Suave,
    Role::White,
    Role::Off,
];

/// The first step is never a half beat: the engine skips that one on Start (see `Engine::pulse`).
fn sequence(lens: &[StepLen], moment: Moment, fade: bool) -> Sequence {
    Sequence {
        name: "fixture".into(),
        fade,
        moment,
        steps: lens
            .iter()
            .enumerate()
            .map(|(i, len)| Step {
                len: *len,
                roles: vec![ROLES[i]],
            })
            .collect(),
    }
}

fn name(l: StepLen) -> &'static str {
    match l {
        Half => "half",
        One => "one",
        Two => "two",
        Four => "four",
    }
}

/// Runs the engine and reads, 300 ms after each half-beat pulse, which step the strip shows and how
/// far a fade has got: `(step, t)` with `t` in (0, 1]. Calm is forced right after Start.
fn shown_steps(seq: &Sequence) -> Vec<(usize, f64)> {
    let calm = seq.moment == Moment::Calm;
    let mut lib = Library::builtin(SEGMENTS);
    lib.sequences.push((1, seq.clone()));
    let track = if calm {
        &mut lib.sets[0].calm
    } else {
        &mut lib.sets[0].rhythm
    };
    track.sequences = vec![SeqRef::Id(1)];
    track.order = light_core::library::Order::InOrder;
    lib.sets[0].palettes = vec![1];
    let mut e = Engine::new(EngineConfig {
        segments: SEGMENTS,
        phrase: PhraseBars::Four,
        lead_us: 100_000,
        brightness_pct: 100,
        lights_on: true,
    });
    let palette = lib.current_palette();
    let mut strip: Vec<Colour> = vec![None; usize::from(SEGMENTS)];
    let mut now = T0;
    let take = |strip: &mut Vec<Colour>, cmds: Vec<PaintCommand>| {
        for c in cmds {
            apply(strip, &c);
        }
    };
    let c = e.set_library(now, &lib);
    take(&mut strip, c);
    let c = e.on_midi(now, 0xFA, |_| None);
    take(&mut strip, c);
    if calm {
        let c = e.command(now, UiCommand::Force(Moment::Calm));
        take(&mut strip, c);
    }
    let mut out = Vec::new();
    for k in 0..HALF_BEATS * 12 {
        while let Some(d) = e.next_deadline_us().filter(|d| *d <= now) {
            let c = e.on_deadline(d);
            take(&mut strip, c);
        }
        if k > 0 {
            let c = e.on_midi(now, 0xF8, |_| None);
            take(&mut strip, c);
        }
        let next = now + TICK_US;
        while let Some(d) = e.next_deadline_us().filter(|d| *d < next) {
            let c = e.on_deadline(d);
            take(&mut strip, c);
        }
        now = next;
        if k % 12 == 7 {
            out.push(read_frame(seq, &palette, strip[0]));
        }
    }
    out
}

/// Which step the strip colour belongs to, and at which fade position: the target itself (t = 1) or
/// `mix(previous step, step, j, frames)`.
fn read_frame(seq: &Sequence, palette: &light_core::model::Palette, shown: Colour) -> (usize, f64) {
    let colour = |i: usize| light_core::model::resolve(seq.steps[i].roles[0], palette, 100);
    let n = seq.steps.len();
    (0..n)
        .find_map(|i| {
            if colour(i) == shown {
                return Some((i, 1.0));
            }
            let frames = fade::frames(seq.steps[i].len.half_beats());
            (1..frames)
                .find(|j| fade::mix(colour((i + n - 1) % n), colour(i), *j, frames) == shown)
                .map(|j| (i, f64::from(j) / f64::from(frames)))
        })
        .expect("strip shows a step colour or a fade frame")
}

fn fixture() -> String {
    let cases: Vec<_> = [
        (vec![One, Two, Half, Half, Four], Moment::Rhythm, true),
        (vec![Two, Half, One, Half, Four], Moment::Rhythm, true),
        (vec![Four, One, Two, Half, Half], Moment::Rhythm, true),
        (vec![Four, Two, One, One, Four], Moment::Calm, true),
        (vec![Four, Four, Two, Two], Moment::Calm, true),
        (vec![Four, Two, One, One, Four], Moment::Calm, false),
    ]
    .into_iter()
    .map(|(lens, moment, fade)| {
        let shown: Vec<_> = shown_steps(&sequence(&lens, moment, fade))
            .into_iter()
            .enumerate()
            .map(|(half, (idx, t))| (half, idx, t))
            .collect();
        let steps: Vec<_> = lens.iter().map(|l| name(*l)).collect();
        let moment = if moment == Moment::Calm {
            "calm"
        } else {
            "rhythm"
        };
        serde_json::json!({ "steps": steps, "moment": moment, "fade": fade, "shown": shown })
    })
    .collect();
    let mut s = serde_json::to_string_pretty(&cases).expect("json");
    s.push('\n');
    s
}

#[test]
fn player_fixture_matches_the_engine() {
    let now = fixture();
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::write(FIXTURE, &now).expect("write fixture");
    }
    let committed = std::fs::read_to_string(FIXTURE).unwrap_or_default();
    assert_eq!(
        committed, now,
        "engine and committed fixture differ; rerun with UPDATE_FIXTURES=1 if the change is intended"
    );
}
