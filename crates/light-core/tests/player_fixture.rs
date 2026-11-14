#![allow(clippy::expect_used)] // helpers are outside allow-expect-in-tests
//! The editor's simulation (`ui/src/editor/sequences/player.ts`) must pick the same step as the
//! engine for the same half-beat count. The fixture is what the real engine shows; regenerate it with
//! `UPDATE_FIXTURES=1 cargo test -p light-core --test player_fixture`.
use light_core::clock::PhraseBars;
use light_core::detect::Moment;
use light_core::engine::{Engine, EngineConfig};
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
fn sequence(lens: &[StepLen]) -> Sequence {
    Sequence {
        name: "fixture".into(),
        fade: true,
        moment: Moment::Rhythm,
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

/// Runs the engine and reads, 300 ms after each half-beat pulse, which step the strip shows.
fn shown_steps(seq: &Sequence) -> Vec<usize> {
    let mut lib = Library::builtin(SEGMENTS);
    lib.sequences[0].1 = seq.clone();
    lib.sets[0].rhythm.sequences = vec![SeqRef::Id(1)];
    lib.sets[0].rhythm.order = light_core::library::Order::InOrder;
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
            out.push(
                ROLES
                    .iter()
                    .position(|r| light_core::model::resolve(*r, &palette, 100) == strip[0])
                    .expect("strip shows a step colour"),
            );
        }
    }
    out
}

fn fixture() -> String {
    let cases: Vec<_> = [
        vec![One, Two, Half, Half, Four],
        vec![Two, Half, One, Half, Four],
        vec![Four, One, Two, Half, Half],
    ]
    .into_iter()
    .map(|lens| {
        let pairs: Vec<_> = shown_steps(&sequence(&lens))
            .into_iter()
            .enumerate()
            .collect();
        let steps: Vec<_> = lens.iter().map(|l| name(*l)).collect();
        serde_json::json!({ "steps": steps, "shown": pairs })
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
