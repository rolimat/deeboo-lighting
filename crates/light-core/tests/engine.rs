use light_core::clock::PhraseBars;
use light_core::detect::Moment::{self, BuildUp, Calm, Rhythm};
use light_core::engine::{Engine, EngineConfig, EngineStatus, UiCommand};
use light_core::paint::PaintCommand;

const LEAD: u64 = 100_000;
const T0: u64 = 1_000_000;

struct Sim {
    e: Engine,
    now: u64,
    tick_us: u64,
    sends: Vec<(u64, PaintCommand)>,
    /// Status after each beat pulse, with its time.
    beats: Vec<(u64, EngineStatus)>,
    detect_calls: u64,
}

impl Sim {
    fn new(tick_us: u64) -> Self {
        let e = Engine::new(EngineConfig {
            segments: 4,
            phrase: PhraseBars::Four,
            lead_us: LEAD,
            brightness_pct: 100,
            lights_on: true,
        });
        Self {
            e,
            now: T0,
            tick_us,
            sends: vec![],
            beats: vec![],
            detect_calls: 0,
        }
    }

    fn record(&mut self, at: u64, cmds: Vec<PaintCommand>) {
        self.sends.extend(cmds.into_iter().map(|c| (at, c)));
    }

    fn deadlines_until(&mut self, t: u64) {
        while let Some(d) = self.e.next_deadline_us() {
            if d > t {
                break;
            }
            let d = d.max(self.now);
            self.now = d;
            let c = self.e.on_deadline(d);
            self.record(d, c);
        }
    }

    fn byte(&mut self, status: u8, detect: &mut dyn FnMut(u64) -> Option<Moment>) {
        let t = self.now;
        let mut called = 0;
        let calls = self.detect_calls;
        let c = self.e.on_midi(t, status, |_| {
            called += 1;
            detect(calls)
        });
        self.detect_calls += called;
        self.record(t, c);
        if called > 0 {
            self.beats.push((t, self.e.status()));
        }
    }

    fn start(&mut self) {
        self.byte(0xFA, &mut |_| None);
    }

    fn ticks(&mut self, n: u64, detect: &mut dyn FnMut(u64) -> Option<Moment>) {
        for _ in 0..n {
            self.deadlines_until(self.now);
            self.byte(0xF8, detect);
            let next = self.now + self.tick_us;
            self.deadlines_until(next - 1);
            self.now = next;
        }
    }

    fn beats_of(&mut self, n: u64, detect: &mut dyn FnMut(u64) -> Option<Moment>) {
        self.ticks(24 * n, detect);
    }

    fn cmd(&mut self, c: UiCommand) {
        let out = self.e.command(self.now, c);
        self.record(self.now, out);
    }

    fn sends_in(&self, from: u64, to: u64) -> Vec<(u64, PaintCommand)> {
        self.sends
            .iter()
            .filter(|(t, _)| *t >= from && *t < to)
            .cloned()
            .collect()
    }
}

fn none(_: u64) -> Option<Moment> {
    None
}

// 125 BPM: 20 ms per tick, 480 ms per beat, 240 ms per half-beat.
fn sim125() -> Sim {
    Sim::new(20_000)
}

#[test]
fn rhythm_steps_are_sent_lead_before_each_beat() {
    let mut s = sim125();
    s.start();
    s.beats_of(3, &mut none);
    // Step 1 is shown on beat 1 (second beat) and step 0 again on beat 2: first message LEAD before.
    for beat in [1, 2] {
        let at = T0 + beat * 480_000 - LEAD;
        assert!(
            s.sends.iter().any(|(t, _)| *t == at),
            "{beat} {:?}",
            s.sends
        );
    }
}

#[test]
fn steps_land_on_beats_after_start_and_after_a_drop() {
    let mut s = sim125();
    s.start();
    // Step 1 is on the Start beat itself.
    s.beats_of(14, &mut |beat| match beat {
        4..=9 => Some(Calm),
        _ => Some(Rhythm),
    });
    assert_eq!(s.sends.first().map(|(t, _)| *t), Some(T0));
    // Calm decided at beat 4 starts on beat 5; Rhythm decided at beat 10 starts on beat 11.
    // Its colour goes out within a message or two of the lead before the beat (never half a beat early/late).
    let first_with = |pred: fn(&PaintCommand) -> bool| {
        let from = T0 + 4 * 480_000;
        s.sends_in(from, u64::MAX)
            .into_iter()
            .find(|(_, c)| pred(c))
    };
    let suave =
        |c: &PaintCommand| matches!(c, PaintCommand::Pencil { color, .. } if color.s == 350);
    let t = first_with(suave).map(|(t, _)| t).unwrap();
    assert!(
        (T0 + 5 * 480_000 - LEAD..T0 + 5 * 480_000 + 250_000).contains(&t),
        "{t}"
    );
    let red = |c: &PaintCommand| matches!(c, PaintCommand::Pencil { color, .. } if color.h == 0 && color.s == 1000);
    let t = s
        .sends_in(T0 + 10 * 480_000, u64::MAX)
        .into_iter()
        .find(|(_, c)| red(c))
        .map(|(t, _)| t)
        .unwrap();
    assert!(
        (T0 + 11 * 480_000 - LEAD..T0 + 11 * 480_000 + 250_000).contains(&t),
        "{t}"
    );
}

#[test]
fn stop_sends_the_rest_scene_once() {
    let mut s = sim125();
    s.start();
    s.beats_of(2, &mut none);
    let stop_at = s.now;
    s.byte(0xFC, &mut |_| None);
    s.deadlines_until(stop_at + 10_000_000);
    let after = s.sends_in(stop_at, u64::MAX);
    assert_eq!(after.len(), 1, "{after:?}");
    assert!(matches!(after[0].1, PaintCommand::Bucket(h) if h.s == 350));
    assert_eq!(s.e.next_deadline_us(), None);
    assert!(!s.e.status().playing);
}

#[test]
fn lights_off_erases_and_stays_dark() {
    let mut s = sim125();
    s.start();
    s.beats_of(2, &mut none);
    s.ticks(12, &mut none); // let step 0 finish going out
    let off_at = s.now;
    s.cmd(UiCommand::Lights(false));
    s.beats_of(4, &mut none);
    let after = s.sends_in(off_at, u64::MAX);
    assert_eq!(after.len(), 1, "{after:?}");
    assert!(matches!(&after[0].1, PaintCommand::Eraser { segments } if segments.len() == 4));
    assert!(!s.e.status().lights_on);
}

#[test]
fn forced_calm_starts_calm_sequence_next_beat() {
    let mut s = sim125();
    s.start();
    s.beats_of(3, &mut none);
    s.cmd(UiCommand::Force(Calm));
    let pulse = s.now;
    s.ticks(1, &mut none);
    assert_eq!(s.e.status().moment, Calm);
    assert!(!s.e.status().auto);
    s.ticks(23, &mut none);
    // Shown at the next half-beat, not the next full beat: first message half-beat minus lead.
    let first = s.sends_in(pulse, pulse + 480_000);
    assert_eq!(
        first.first().map(|(t, _)| *t),
        Some(pulse + 240_000 - LEAD),
        "{first:?}"
    );
    // Calm step 0 (Suave on segments 1 and 3): the odd-segment pencil follows the erase.
    assert!(
        first.iter().any(
            |(_, c)| matches!(c, PaintCommand::Pencil { color, segments }
            if color.s == 350 && segments == &[1, 3])
        ),
        "{first:?}"
    );
}

#[test]
fn detected_buildup_starts_at_bar_start() {
    let mut s = sim125();
    s.start();
    s.beats_of(12, &mut |beat| (beat >= 5).then_some(BuildUp));
    let at = |b: usize| s.beats.get(b).map(|(_, st)| st.moment);
    assert_eq!(at(5), Some(Rhythm));
    assert_eq!(at(6), Some(Rhythm));
    // Decided on the last beat before bar 3 starts.
    assert_eq!(at(7), Some(BuildUp));
    assert_eq!(at(9), Some(BuildUp));
}

#[test]
fn drop_prediction_lands_on_phrase_start() {
    let mut s = sim125();
    s.start();
    s.beats_of(19, &mut |beat| (4..=20).contains(&beat).then_some(BuildUp));
    let st = |b: usize| s.beats.get(b).map(|(_, st)| st.clone()).unwrap();
    assert_eq!(st(14).moment, BuildUp);
    // Decided on the last beat of the phrase, shown from the phrase start.
    assert_eq!((st(15).moment, st(15).predicted), (Rhythm, true));
    // The detector lags: the prediction holds, then reverts when the drop is still not reported.
    assert_eq!(st(16).moment, Rhythm);
    assert_eq!(st(17).moment, Rhythm);
    assert_eq!((st(18).moment, st(18).predicted), (BuildUp, false));
    // The drop's first Rhythm message is scheduled for the phrase start (t15 + 480 ms).
    let t15 = s.beats.get(15).map(|(t, _)| *t).unwrap();
    assert_eq!(
        s.sends_in(t15 + 200_000, t15 + 480_000)
            .first()
            .map(|(t, _)| *t),
        Some(t15 + 480_000 - LEAD)
    );
}

// Real lag: the drop starts at beat 16; the detector reports it from the call at beat 18.
#[test]
fn predicted_drop_holds_through_detector_lag() {
    let mut s = sim125();
    s.start();
    s.beats_of(22, &mut |beat| match beat {
        4..=17 => Some(BuildUp),
        _ => Some(Rhythm),
    });
    let st = |b: usize| s.beats.get(b).map(|(_, st)| st.clone()).unwrap();
    assert_eq!(st(14).moment, BuildUp);
    for b in 15..=21 {
        assert_eq!(st(b).moment, Rhythm, "beat {b}");
    }
    assert!(!st(18).predicted);
}

#[test]
fn unconfirmed_prediction_reverts_after_the_lag() {
    let mut s = sim125();
    s.start();
    s.beats_of(22, &mut |beat| (4..=21).contains(&beat).then_some(BuildUp));
    let st = |b: usize| s.beats.get(b).map(|(_, st)| st.clone()).unwrap();
    for b in 15..=17 {
        assert_eq!(st(b).moment, Rhythm, "beat {b}");
    }
    assert_eq!(st(18).moment, BuildUp);
}

#[test]
fn start_mid_play_restarts_sequence_at_beat_one() {
    let mut s = sim125();
    s.start();
    s.beats_of(1, &mut none);
    s.ticks(12, &mut none); // half-beat: sequence now on its second step
    s.start();
    let pulse = s.now;
    s.ticks(1, &mut none);
    assert_eq!(s.e.status().pos.map(|p| p.beat), Some(0));
    s.ticks(7, &mut none);
    let first = s.sends_in(pulse, pulse + 480_000);
    // Step 1 goes out on the Start pulse itself.
    assert_eq!(first[0].0, pulse, "{first:?}");
    assert!(
        first.iter().any(
            |(_, c)| matches!(c, PaintCommand::Pencil { color, segments }
            if color.h == 0 && segments == &[1, 2])
        ),
        "{first:?}"
    );
}

#[test]
fn half_beat_steps_at_140_bpm_never_exceed_one_message_per_gap() {
    let mut s = Sim::new(17_857);
    s.start();
    s.beats_of(12, &mut none);
    assert!(s.sends.len() > 10);
    for w in s.sends.windows(2) {
        let (a, b) = (w.first().unwrap().0, w.get(1).unwrap().0);
        assert!(b - a >= 125_000, "{a} {b}");
    }
}

#[test]
fn detect_called_once_per_beat() {
    let mut s = sim125();
    s.start();
    s.ticks(24 * 4, &mut none);
    // Pulses at ticks 0, 12, 24 ... 84: 4 beats and 4 half pulses.
    assert_eq!(s.detect_calls, 4);
}

#[test]
fn idle_engine_has_no_deadline() {
    let e = Engine::new(EngineConfig {
        segments: 4,
        phrase: PhraseBars::Four,
        lead_us: LEAD,
        brightness_pct: 100,
        lights_on: true,
    });
    assert_eq!(e.next_deadline_us(), None);
}

fn first_bucket_at(s: &Sim, hue: u16) -> Option<u64> {
    s.sends
        .iter()
        .find(|(_, c)| matches!(c, PaintCommand::Bucket(h) if h.h == hue))
        .map(|(t, _)| *t)
}

#[test]
fn buildup_detected_before_a_bar_start_shows_from_that_bar_start() {
    let mut s = sim125();
    s.start();
    s.beats_of(6, &mut |beat| (beat >= 3).then_some(BuildUp));
    let (t3, st3) = s.beats.get(3).cloned().unwrap();
    assert_eq!(st3.pos.map(|p| p.beat_in_bar), Some(4));
    assert_eq!(st3.moment, BuildUp);
    // Bar 2 starts at t3 + 480 ms; its first flash message goes out one lead earlier.
    assert_eq!(first_bucket_at(&s, 240), Some(t3 + 480_000 - LEAD));
}

#[test]
fn buildup_detected_on_a_bar_start_waits_for_the_next_bar_start() {
    let mut s = sim125();
    s.start();
    s.beats_of(10, &mut |beat| (beat >= 4).then_some(BuildUp));
    let at = |b: usize| s.beats.get(b).map(|(_, st)| st.moment);
    assert_eq!(at(4), Some(Rhythm));
    assert_eq!(at(6), Some(Rhythm));
    assert_eq!(at(7), Some(BuildUp));
    let t7 = s.beats.get(7).map(|(t, _)| *t).unwrap();
    assert_eq!(first_bucket_at(&s, 240), Some(t7 + 480_000 - LEAD));
}

#[test]
fn quick_buildup_waits_for_next_bar() {
    // Detected on a phrase's first beat (quick entry): the strip still waits for the next bar start.
    let mut s = sim125();
    s.start();
    s.beats_of(40, &mut |beat| (beat >= 32).then_some(BuildUp));
    let at = |b: usize| s.beats.get(b).map(|(_, st)| st.moment);
    assert_eq!(at(32), Some(Rhythm));
    assert_eq!(at(34), Some(Rhythm));
    assert_eq!(at(35), Some(BuildUp));
}

fn apply_all(
    sends: &[(u64, PaintCommand)],
    mut state: Vec<light_core::model::Colour>,
) -> Vec<light_core::model::Colour> {
    for (_, c) in sends {
        light_core::scene::apply(&mut state, c);
    }
    state
}

#[allow(clippy::unnecessary_wraps)] // builds a `Colour`
fn hsv(h: u16, s: u16, v: u16) -> Option<light_core::paint::Hsv> {
    Some(light_core::paint::Hsv { h, s, v })
}

#[test]
fn strip_reconnected_resends_the_full_scene() {
    // 60 BPM with a 300 ms lead: both messages of a step fit before the next half pulse.
    let mut s = Sim::new(41_666);
    s.cmd(UiCommand::LeadMs(300));
    s.start();
    s.beats_of(2, &mut none);
    s.ticks(1, &mut none);
    let from = s.now;
    s.e.strip_reconnected();
    s.ticks(22, &mut none);
    // Rhythm step 1 (dark, dark, white, white), the next beat's, rebuilt on a strip in an unknown state.
    let got = apply_all(&s.sends_in(from, u64::MAX), vec![hsv(77, 77, 77); 4]);
    let white = hsv(0, 0, 1000);
    assert_eq!(got, vec![None, None, white, white]);
}

#[test]
fn brightness_applies_from_the_next_step() {
    let mut s = sim125();
    s.start();
    s.beats_of(2, &mut none);
    s.ticks(2, &mut none);
    let at = s.now;
    s.cmd(UiCommand::Brightness(50));
    assert_eq!(s.sends_in(at, at + 1).len(), 0);
    s.ticks(84, &mut none);
    let state = apply_all(&s.sends, vec![None; 4]);
    let white = hsv(0, 0, 500);
    assert!(
        state == vec![None, None, white, white]
            || state == vec![hsv(0, 1000, 500), hsv(0, 1000, 500), None, None],
        "{state:?}"
    );
    assert!(s.sends_in(at, u64::MAX).iter().all(|(_, c)| match c {
        PaintCommand::Pencil { color, .. } => color.v == 500,
        PaintCommand::Bucket(c) => c.v == 500,
        PaintCommand::Eraser { .. } => true,
    }));
}

#[test]
fn phrase_change_moves_the_phrase_boundary() {
    let mut s = sim125();
    s.cmd(UiCommand::Phrase(PhraseBars::Eight));
    s.start();
    s.beats_of(10, &mut |beat| (beat >= 4).then_some(BuildUp));
    s.cmd(UiCommand::Phrase(PhraseBars::Four));
    s.beats_of(12, &mut |beat| (beat >= 4).then_some(BuildUp));
    let st = |b: usize| s.beats.get(b).map(|(_, st)| st.clone()).unwrap();
    let bar = |b: usize| st(b).pos.map(|p| p.bar_in_phrase);
    assert_eq!(bar(9), Some(3)); // 8-bar phrase so far
    assert_eq!(bar(16), Some(1)); // 4-bar phrase: wraps at beat 16
    assert_eq!(bar(20), Some(2));
    assert!(!st(14).predicted);
    assert!(st(15).predicted);
    assert_eq!(st(15).moment, Rhythm);
}

#[test]
fn lead_ms_changes_send_times() {
    // 40 BPM: 1.5 s beat, 750 ms half-beat.
    let mut s = Sim::new(62_500);
    s.cmd(UiCommand::LeadMs(300));
    s.start();
    s.beats_of(2, &mut none);
    // The half pulse before beat 1 plans its step, sent one lead before T0 + 1500 ms.
    assert!(s.sends.iter().any(|(t, _)| *t == T0 + 1_500_000 - 300_000));

    let mut c = Sim::new(62_500);
    c.cmd(UiCommand::LeadMs(900));
    c.start();
    c.beats_of(2, &mut none);
    // Clamped to 500 ms: not sent at the planning pulse (T0 + 750 ms), but 500 ms before the beat.
    assert!(c.sends.iter().all(|(t, _)| *t != T0 + 750_000));
    assert!(c.sends.iter().any(|(t, _)| *t == T0 + 1_500_000 - 500_000));
}

/// The built-in library reduced to one palette and the first sequence of each moment, in order.
fn plain(n: u8) -> light_core::library::Library {
    use light_core::library::{Order, SeqRef};
    let mut lib = light_core::library::Library::builtin(n);
    lib.palettes.truncate(1);
    lib.active_palette = Some(1);
    let set = &mut lib.sets[0];
    set.palettes = vec![1];
    for (ms, id) in [
        (&mut set.rhythm, 1),
        (&mut set.calm, 5),
        (&mut set.buildup, 8),
    ] {
        ms.sequences = vec![SeqRef::Id(id)];
        ms.order = Order::InOrder;
    }
    lib
}

fn lib4(principal_h: u16) -> light_core::library::Library {
    let mut lib = plain(4);
    lib.palettes[0].1.principal.h = principal_h;
    lib
}

fn hue_sends(s: &Sim, hue: u16) -> Vec<u64> {
    s.sends
        .iter()
        .filter(|(_, c)| match c {
            PaintCommand::Bucket(h) => h.h == hue,
            PaintCommand::Pencil { color, .. } => color.h == hue,
            PaintCommand::Eraser { .. } => false,
        })
        .map(|(t, _)| *t)
        .collect()
}

#[test]
fn library_change_applies_at_next_bar() {
    let mut s = sim125();
    s.start();
    s.beats_of(2, &mut none);
    let at = s.now;
    let out = s.e.set_library(at, &lib4(100));
    assert!(out.is_empty(), "no burst on save: {out:?}");
    s.beats_of(6, &mut none);
    // Bar 2 starts one beat after beat 3's pulse; its first message goes out one lead earlier.
    let t3 = s.beats.get(3).map(|(t, _)| *t).unwrap();
    let hue100 = hue_sends(&s, 100);
    // The step before the bar start is the old palette's; nothing new goes out earlier.
    let first = hue100.first().copied().unwrap();
    assert!(
        (t3 + 480_000 - LEAD..t3 + 480_000 + LEAD).contains(&first),
        "{hue100:?}"
    );
}

#[test]
fn mismatched_segments_fall_back_to_builtin() {
    let mut s = sim125();
    // A library built for 10 segments on a 4-segment strip: its sequences are ignored.
    let mut lib = light_core::library::Library::builtin(10);
    lib.palettes[0].1.principal.h = 100;
    s.cmd(UiCommand::Lights(true));
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(4, &mut none);
    // The built-in Halves sequence for 4 segments, in the library's palette.
    let pencils: Vec<_> = s
        .sends
        .iter()
        .filter_map(|(_, c)| match c {
            PaintCommand::Pencil { color, segments } => Some((color.h, segments.clone())),
            _ => None,
        })
        .collect();
    assert!(pencils.contains(&(100, vec![1, 2])), "{pencils:?}");
    assert!(pencils.contains(&(0, vec![3, 4])), "{pencils:?}");
}

#[test]
fn matching_library_sequence_is_played() {
    use light_core::model::Role;
    let mut s = sim125();
    let mut lib = lib4(100);
    for st in &mut lib.sequences[0].1.steps {
        st.roles = vec![Role::Contraste; 4];
    }
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(2, &mut none);
    let got = apply_all(&s.sends, vec![None; 4]);
    assert_eq!(got, vec![hsv(280, 900, 950); 4]);
}

#[test]
fn generated_sequence_plays_and_changes() {
    use light_core::library::{Interval, Library, SeqRef};
    let mut lib = Library::builtin(4);
    let ms = &mut lib.sets[0].rhythm;
    ms.sequences = vec![SeqRef::Generated];
    ms.interval = Interval::Bars(1);
    let mut s = sim125();
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(64, &mut none);
    // The messages of each bar (windows start one lead before the bar): built-in Halves repeats
    // identically, a generated sequence rotating every bar does not.
    let bars: Vec<String> = (1..15u64)
        .map(|bar| {
            let from = T0 + bar * 4 * 480_000 - LEAD;
            format!(
                "{:?}",
                s.sends_in(from, from + 4 * 480_000)
                    .iter()
                    .map(|(_, c)| c)
                    .collect::<Vec<_>>()
            )
        })
        .collect();
    assert!(bars.iter().any(|b| *b != bars[0]), "{bars:?}");
}

#[test]
fn palette_changes_on_drop_when_at_drop() {
    let mut lib = plain(4);
    lib.active_palette = None;
    let mut p2 = lib.palettes[0].1.clone();
    p2.principal.h = 100;
    lib.palettes.push((2, p2));
    lib.sets[0].palettes = vec![1, 2];
    let mut s = sim125();
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(2, &mut none);
    s.cmd(UiCommand::Force(BuildUp));
    s.beats_of(8, &mut none);
    s.cmd(UiCommand::Force(Rhythm));
    s.beats_of(8, &mut none);
    assert_ne!(hue_sends(&s, 100), Vec::<u64>::new());
}

/// Rhythm alternates two sequences, `Suave` (hue 120) then `Contraste` (hue 240), every phrase.
fn two_seq_lib() -> light_core::library::Library {
    use light_core::library::Interval;
    use light_core::model::Role;
    let mut lib = plain(4);
    let mut q2 = lib.sequences[0].1.clone();
    for (q, role) in [
        (&mut lib.sequences[0].1, Role::Suave),
        (&mut q2, Role::Contraste),
    ] {
        for st in &mut q.steps {
            st.roles = vec![role; 4];
        }
    }
    lib.sequences.push((99, q2));
    lib.sets[0].rhythm.sequences = vec![
        light_core::library::SeqRef::Id(1),
        light_core::library::SeqRef::Id(99),
    ];
    lib.sets[0].rhythm.interval = Interval::Bars(4);
    lib
}

fn colour_at(s: &Sim, hue: u16, from: u64, to: u64) -> bool {
    s.sends_in(from, to).iter().any(|(_, c)| match c {
        PaintCommand::Bucket(h) => h.h == hue,
        PaintCommand::Pencil { color, .. } => color.h == hue,
        PaintCommand::Eraser { .. } => false,
    })
}

#[test]
fn rotation_continues_after_a_start_while_playing() {
    let mut s = sim125();
    let lib = two_seq_lib();
    let suave = lib.palettes[0].1.suave.h;
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(20, &mut none); // changed to Contraste at beat 16
    s.start();
    let restart = s.now;
    s.beats_of(20, &mut none);
    let beat = 480_000;
    assert!(
        colour_at(&s, suave, restart + 15 * beat, restart + 20 * beat),
        "back to Suave one phrase after the restart"
    );
}
/// The pencil segment lists sent: the pattern, whatever the palette.
fn shapes(s: &Sim) -> Vec<Vec<u8>> {
    s.sends
        .iter()
        .filter_map(|(_, c)| match c {
            PaintCommand::Pencil { segments, .. } => Some(segments.clone()),
            _ => None,
        })
        .collect()
}

fn gen_lib() -> light_core::library::Library {
    use light_core::library::{Interval, Library, SeqRef};
    let mut lib = Library::builtin(4);
    let ms = &mut lib.sets[0].rhythm;
    ms.sequences = vec![SeqRef::Generated];
    ms.order = light_core::library::Order::Fixed;
    ms.interval = Interval::Bars(16);
    lib.sets[0].palette_order = light_core::library::Order::Fixed;
    lib
}

#[test]
fn redelivering_the_same_library_keeps_the_playing_sequence() {
    let run = |redeliver: bool| {
        let mut s = sim125();
        s.e.set_library(s.now, &gen_lib());
        s.start();
        s.beats_of(2, &mut none);
        if redeliver {
            s.e.set_library(s.now, &gen_lib());
        }
        s.beats_of(14, &mut none);
        shapes(&s)
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn only_the_pinned_palette_changing_keeps_the_sequence() {
    let mut lib2 = gen_lib();
    let mut p2 = lib2.palettes[0].1.clone();
    p2.principal.h = 100;
    p2.contraste.h = 100;
    p2.suave.h = 100;
    lib2.palettes.push((99, p2));
    let mut lib3 = lib2.clone();
    lib3.active_palette = Some(99);
    let run = |second: Option<&light_core::library::Library>| {
        let mut s = sim125();
        s.e.set_library(s.now, &lib2);
        s.start();
        s.beats_of(2, &mut none);
        if let Some(l) = second {
            s.e.set_library(s.now, l);
        }
        s.beats_of(14, &mut none);
        s
    };
    let (a, b) = (run(None), run(Some(&lib3)));
    assert_eq!(shapes(&a), shapes(&b));
    assert_eq!(hue_sends(&a, 100), Vec::<u64>::new());
    assert_ne!(hue_sends(&b, 100), Vec::<u64>::new());
}

#[test]
fn edited_shown_palette_shows_from_the_next_bar() {
    let mut lib = plain(4);
    lib.active_palette = None;
    let mut p2 = lib.palettes[0].1.clone();
    p2.principal.h = 100;
    lib.palettes.push((2, p2));
    lib.sets[0].palettes = vec![1, 2];
    let mut edited = lib.clone();
    edited.palettes[0].1.principal.h = 77;
    edited.palettes[0].1.contraste.h = 77;
    edited.palettes[0].1.suave.h = 77;
    let mut s = sim125();
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(2, &mut none);
    s.e.set_library(s.now, &edited);
    s.beats_of(6, &mut none);
    assert_ne!(hue_sends(&s, 77), Vec::<u64>::new());
    // The rotation position is unchanged: the next drop goes to palette 2.
    s.cmd(UiCommand::Force(BuildUp));
    s.beats_of(8, &mut none);
    s.cmd(UiCommand::Force(Rhythm));
    s.beats_of(8, &mut none);
    assert_ne!(hue_sends(&s, 100), Vec::<u64>::new());
}

#[test]
fn edited_playing_sequence_shows_from_the_next_bar() {
    use light_core::model::Role;
    let lib = plain(4);
    let mut edited = lib.clone();
    for st in &mut edited.sequences[0].1.steps {
        st.roles = vec![Role::Contraste; 4];
    }
    let mut s = sim125();
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(2, &mut none);
    s.e.set_library(s.now, &edited);
    s.beats_of(6, &mut none);
    let got = apply_all(&s.sends, vec![None; 4]);
    assert_eq!(got, vec![hsv(280, 900, 950); 4]);
}

/// `plain` with a second palette (principal hue 100), in Auto, rotating as `change` says.
fn two_pal_lib(change: light_core::library::PaletteChange) -> light_core::library::Library {
    let mut lib = plain(4);
    let mut p2 = lib.palettes[0].1.clone();
    p2.principal.h = 100;
    lib.palettes.push((2, p2));
    lib.active_palette = None;
    lib.sets[0].palettes = vec![1, 2];
    lib.sets[0].palette_change = change;
    lib
}

fn force_a_drop(s: &mut Sim) {
    s.cmd(UiCommand::Force(BuildUp));
    s.beats_of(8, &mut none);
    s.cmd(UiCommand::Force(Rhythm));
    s.beats_of(8, &mut none);
}

#[test]
fn palette_rotates_every_n_bars_in_auto() {
    use light_core::library::PaletteChange::Bars;
    let mut s = sim125();
    s.e.set_library(s.now, &two_pal_lib(Bars(2)));
    s.start();
    s.beats_of(6, &mut none);
    assert_eq!(hue_sends(&s, 100), Vec::<u64>::new());
    s.beats_of(6, &mut none);
    assert_ne!(hue_sends(&s, 100), Vec::<u64>::new());
    assert_eq!(s.e.status().palette, Some(2));
}

#[test]
fn a_drop_does_not_change_the_palette_when_the_set_changes_by_bars() {
    use light_core::library::PaletteChange::Bars;
    let mut s = sim125();
    s.e.set_library(s.now, &two_pal_lib(Bars(16)));
    s.start();
    s.beats_of(2, &mut none);
    force_a_drop(&mut s);
    assert_eq!(hue_sends(&s, 100), Vec::<u64>::new());
}

#[test]
fn manual_palette_survives_drops_until_auto() {
    use light_core::library::PaletteChange::AtDrop;
    let mut pinned = two_pal_lib(AtDrop);
    pinned.active_palette = Some(1);
    let mut s = sim125();
    s.e.set_library(s.now, &pinned);
    s.start();
    s.beats_of(2, &mut none);
    force_a_drop(&mut s);
    assert_eq!(hue_sends(&s, 100), Vec::<u64>::new());
    let st = s.e.status();
    assert!(!st.palette_auto);
    assert_eq!(st.palette, Some(1));
    // Back to Auto: the rotation (which kept counting drops) takes over.
    s.e.set_library(s.now, &two_pal_lib(AtDrop));
    s.beats_of(8, &mut none);
    assert!(s.e.status().palette_auto);
    assert_ne!(hue_sends(&s, 100), Vec::<u64>::new());
}

/// Calm alternates `Suave` (hue 120) and `Contraste` (hue 240) sequences; rhythm plays only Suave.
fn calm_lib(interval: light_core::library::Interval) -> light_core::library::Library {
    use light_core::library::SeqRef;
    let mut lib = two_seq_lib();
    lib.sets[0].rhythm.sequences = vec![SeqRef::Id(1)];
    lib.sets[0].calm.sequences = vec![SeqRef::Id(1), SeqRef::Id(99)];
    lib.sets[0].calm.interval = interval;
    lib
}

#[test]
fn calm_once_per_break_advances_per_entry() {
    let lib = calm_lib(light_core::library::Interval::OncePerBreak);
    let contraste = lib.palettes[0].1.contraste.h;
    let mut s = sim125();
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(2, &mut none);
    s.cmd(UiCommand::Force(Calm));
    s.beats_of(40, &mut none);
    assert_eq!(
        hue_sends(&s, contraste),
        Vec::<u64>::new(),
        "one break, one sequence"
    );
    s.cmd(UiCommand::Force(Rhythm));
    s.beats_of(4, &mut none);
    s.cmd(UiCommand::Force(Calm));
    s.beats_of(8, &mut none);
    assert_ne!(hue_sends(&s, contraste), Vec::<u64>::new());
}

#[test]
fn calm_bars_rotates_inside_a_long_break() {
    let lib = calm_lib(light_core::library::Interval::Bars(2));
    let contraste = lib.palettes[0].1.contraste.h;
    let mut s = sim125();
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(2, &mut none);
    s.cmd(UiCommand::Force(Calm));
    s.beats_of(24, &mut none);
    assert_ne!(hue_sends(&s, contraste), Vec::<u64>::new());
}

#[test]
fn pattern_ref_plays_that_pattern_with_new_params() {
    use light_core::library::{Interval, Library, Order, SeqRef};
    use light_core::patterns::Pattern;
    let mut lib = Library::builtin(4);
    let ms = &mut lib.sets[0].rhythm;
    ms.sequences = vec![SeqRef::Pattern(Pattern::Halves)];
    ms.order = Order::InOrder;
    ms.interval = Interval::Bars(1);
    let mut s = sim125();
    s.e.set_library(s.now, &lib);
    s.start();
    s.beats_of(64, &mut none);
    let seen = shapes(&s);
    assert_ne!(seen.len(), 0);
    assert!(
        seen.iter().all(|p| *p == [1, 2] || *p == [3, 4]),
        "only halves: {seen:?}"
    );
    let bars: Vec<String> = (1..15u64)
        .map(|bar| {
            let from = T0 + bar * 4 * 480_000 - LEAD;
            format!("{:?}", s.sends_in(from, from + 4 * 480_000))
        })
        .collect();
    assert!(bars.iter().any(|b| *b != bars[0]), "fresh params each bar");
}

// ---- palette preview ("Show on strip") ----

fn pv(h: u16) -> light_core::paint::Hsv {
    light_core::paint::Hsv {
        h,
        s: 1000,
        v: 1000,
    }
}

fn preview_pair() -> (light_core::model::Palette, light_core::model::Sequence) {
    use light_core::model::{Palette, Role, Sequence, Step, StepLen};
    let step = |role| Step {
        len: StepLen::One,
        roles: vec![role, Role::Off, Role::Off, Role::Off],
    };
    (
        Palette {
            name: "P".into(),
            principal: pv(111),
            contraste: pv(222),
            suave: pv(333),
        },
        Sequence {
            name: "S".into(),
            fade: true,
            moment: Rhythm,
            steps: vec![step(Role::Principal), step(Role::Contraste)],
        },
    )
}

fn hues(sends: &[(u64, PaintCommand)]) -> Vec<u16> {
    sends
        .iter()
        .filter_map(|(_, c)| match c {
            PaintCommand::Pencil { color, .. } | PaintCommand::Bucket(color) => Some(color.h),
            PaintCommand::Eraser { .. } => None,
        })
        .collect()
}

#[test]
fn preview_free_runs_without_a_clock_then_restores_the_rest_scene() {
    let mut s = sim125();
    let out = s.e.set_preview(s.now, Some(preview_pair()));
    s.record(s.now, out);
    s.deadlines_until(T0 + 2_000_000);
    let shown = hues(&s.sends);
    assert!(shown.contains(&111) && shown.contains(&222), "{shown:?}");
    let mark = s.sends.len();
    s.now = T0 + 2_000_000;
    let out = s.e.set_preview(s.now, None);
    s.record(s.now, out);
    s.deadlines_until(T0 + 4_000_000);
    let after = hues(&s.sends[mark..]);
    assert!(!after.is_empty(), "rest scene repainted");
    assert!(!after.contains(&111) && !after.contains(&222), "{after:?}");
    assert_eq!(s.e.next_deadline_us(), None, "nothing keeps ticking");
}

#[test]
fn preview_that_does_not_fit_the_strip_is_rejected_and_sends_nothing() {
    let mut s = sim125();
    let (palette, mut seq) = preview_pair();
    for step in &mut seq.steps {
        step.roles.pop(); // one role too few for the strip's segments
    }
    let out = s.e.set_preview(s.now, Some((palette, seq)));
    assert_eq!(out, vec![]);
    s.record(s.now, out);
    s.deadlines_until(T0 + 2_000_000);
    assert!(
        s.sends
            .iter()
            .all(|(_, c)| !matches!(c, PaintCommand::Pencil { color, .. } if color.h == 111))
    );
}

#[test]
fn brightness_repaints_the_preview_without_advancing_it() {
    let mut s = sim125();
    let out = s.e.set_preview(s.now, Some(preview_pair()));
    s.record(s.now, out);
    s.deadlines_until(T0 + 600_000); // a step lasts two pulses: 222 is current
    let due = s.e.next_deadline_us();
    s.now = T0 + 600_000;
    let mark = s.sends.len();
    s.cmd(UiCommand::Brightness(50));
    s.deadlines_until(s.now + 100_000);
    assert_eq!(hues(&s.sends[mark..]), vec![222], "same step, not the next");
    assert!(s.sends[mark..].iter().all(|(_, c)| matches!(c,
        PaintCommand::Pencil { color, .. } | PaintCommand::Bucket(color) if color.v == 500)));
    assert_eq!(s.e.next_deadline_us(), due, "cadence untouched");
}

#[test]
fn preview_follows_the_clock_when_it_runs() {
    let mut s = sim125();
    s.start();
    s.beats_of(2, &mut none);
    let out = s.e.set_preview(s.now, Some(preview_pair()));
    s.record(s.now, out);
    let mark = s.sends.len();
    s.beats_of(3, &mut none);
    let shown = hues(&s.sends[mark..]);
    assert!(shown.contains(&111) && shown.contains(&222), "{shown:?}");
    let mark = s.sends.len();
    let out = s.e.set_preview(s.now, None);
    s.record(s.now, out);
    s.beats_of(3, &mut none);
    let after = hues(&s.sends[mark..]);
    assert!(!after.contains(&111) && !after.contains(&222), "{after:?}");
}

#[test]
fn status_reports_the_playing_clip_and_its_progress() {
    let mut s = sim125();
    s.e.set_library(s.now, &two_seq_lib());
    assert_eq!(s.e.status().clip, None, "stopped: nothing plays");
    s.start();
    s.beats_of(2, &mut none);
    let c = s.e.status().clip.expect("a clip plays");
    assert_eq!((c.moment, c.index), (Rhythm, 0));
    assert!(c.total > 0 && c.elapsed < c.total, "{c:?}");
    let first = c.elapsed;
    s.ticks(12, &mut none); // half a beat later
    assert_ne!(s.e.status().clip.map(|c| c.elapsed), Some(first));
    s.beats_of(16, &mut none); // Bars(4) later the second sequence plays
    assert_eq!(s.e.status().clip.map(|c| c.index), Some(1));
    s.cmd(UiCommand::Force(Calm));
    s.beats_of(2, &mut none);
    assert_eq!(s.e.status().clip.map(|c| c.moment), Some(Calm));
}

// ---- latency calibration ----

/// All four segments white (the built-in show only whitens some of them).
fn is_white(c: &PaintCommand) -> bool {
    match c {
        PaintCommand::Bucket(color) => color.s == 0 && color.v > 0,
        PaintCommand::Pencil { color, segments } => {
            color.s == 0 && color.v > 0 && segments.len() == 4
        }
        PaintCommand::Eraser { .. } => false,
    }
}

#[test]
fn calibrate_flashes_on_beats_and_restores_show() {
    let mut s = sim125();
    s.start();
    s.beats_of(2, &mut none);
    let out = s.e.calibrate(s.now, true);
    s.record(s.now, out);
    let mark = s.sends.len();
    s.beats_of(6, &mut none);
    let flashes: Vec<u64> = s.sends[mark..]
        .iter()
        .filter(|(_, c)| is_white(c))
        .map(|(t, _)| *t)
        .collect();
    assert!(flashes.len() >= 4, "{flashes:?}");
    for t in &flashes {
        assert_eq!((t + LEAD - T0) % 480_000, 0, "flash lands on a beat: {t}");
    }
    assert!(
        s.sends[mark..]
            .iter()
            .any(|(_, c)| matches!(c, PaintCommand::Eraser { .. })),
        "dark between beats"
    );
    let out = s.e.calibrate(s.now, false);
    s.record(s.now, out);
    let mark = s.sends.len();
    s.beats_of(6, &mut none);
    assert!(
        s.sends[mark..].iter().all(|(_, c)| !is_white(c)),
        "the show is back"
    );
    assert!(s.sends.len() > mark, "and it paints");
}

#[test]
fn calibrate_without_a_clock_flashes_at_120_bpm() {
    let mut s = sim125();
    let out = s.e.calibrate(s.now, true);
    s.record(s.now, out);
    s.deadlines_until(T0 + 2_100_000);
    let flashes: Vec<u64> = s
        .sends
        .iter()
        .filter(|(_, c)| is_white(c))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(flashes.len(), 5, "{flashes:?}");
    for w in flashes.windows(2) {
        assert_eq!(w[1] - w[0], 500_000, "one flash per beat at 120 BPM");
    }
    assert!(
        s.sends
            .iter()
            .any(|(_, c)| matches!(c, PaintCommand::Eraser { .. }))
    );
    let out = s.e.calibrate(s.now, false);
    s.record(s.now, out);
    let mark = s.sends.len();
    s.deadlines_until(s.now + 2_000_000);
    assert!(s.sends[mark..].iter().all(|(_, c)| !is_white(c)), "ended");
}

#[test]
fn calibrate_keeps_flashing_across_a_clock_start_and_stop() {
    let mut s = sim125();
    let out = s.e.calibrate(s.now, true);
    s.record(s.now, out);
    s.start();
    let mark = s.sends.len();
    s.beats_of(4, &mut none);
    assert!(s.sends[mark..].iter().filter(|(_, c)| is_white(c)).count() >= 3);
    s.byte(0xFC, &mut none); // Stop
    let mark = s.sends.len();
    s.deadlines_until(s.now + 2_000_000);
    assert!(
        s.sends[mark..].iter().any(|(_, c)| is_white(c)),
        "free-running again after Stop"
    );
}

#[test]
fn detect_gets_the_closed_beat() {
    let mut e = Engine::new(EngineConfig {
        segments: 4,
        phrase: PhraseBars::Four,
        lead_us: LEAD,
        brightness_pct: 100,
        lights_on: true,
    });
    let mut seen = Vec::new();
    e.on_midi(T0, 0xFA, |_| None);
    for i in 0..48 {
        e.on_midi(T0 + i * 20_000, 0xF8, |p| {
            seen.push(p);
            None
        });
    }
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], None);
    assert_eq!(seen[1].map(|p| (p.beat, p.half)), Some((0, false)));
}

#[test]
fn brightness_repaints_the_playing_step() {
    let mut s = sim125();
    s.start();
    s.cmd(UiCommand::Force(Calm)); // Calm holds a step for bars: only a resend repaints it
    s.beats_of(6, &mut none);
    s.ticks(2, &mut none);
    let at = s.now;
    s.cmd(UiCommand::Brightness(50));
    s.ticks(24, &mut none);
    let sends = s.sends_in(at, u64::MAX);
    assert!(
        sends.iter().any(|(_, c)| matches!(c,
            PaintCommand::Pencil { color, .. } if color.v == 500)
            || matches!(c, PaintCommand::Bucket(c) if c.v == 500)),
        "{sends:?}"
    );
}

#[test]
fn brightness_repaints_the_rest_scene() {
    let mut s = sim125();
    s.cmd(UiCommand::Lights(true));
    let at = s.now;
    s.cmd(UiCommand::Brightness(50));
    s.deadlines_until(at + 10_000_000);
    let sends = s.sends_in(at, u64::MAX);
    assert!(
        sends
            .iter()
            .any(|(_, c)| matches!(c, PaintCommand::Bucket(h) if h.s == 350 && h.v < 1000)),
        "{sends:?}"
    );
}

#[test]
fn brightness_with_lights_off_sends_nothing() {
    let mut s = sim125();
    s.cmd(UiCommand::Lights(false));
    s.deadlines_until(s.now + 1_000_000);
    let at = s.now;
    s.cmd(UiCommand::Brightness(50));
    s.deadlines_until(at + 10_000_000);
    assert_eq!(s.sends_in(at, u64::MAX), vec![]);
}

#[test]
fn a_reverted_predicted_drop_and_the_real_drop_advance_the_palette_once() {
    use light_core::library::PaletteChange::AtDrop;
    let mut s = sim125();
    s.e.set_library(s.now, &two_pal_lib(AtDrop));
    s.start();
    // Build-up from beat 4; the phrase-start prediction (beat 15) is contradicted at beat 18,
    // and the real drop is reported from beat 28.
    s.beats_of(40, &mut |beat| match beat {
        4..=27 => Some(BuildUp),
        _ => Some(Rhythm),
    });
    assert_eq!(s.e.status().moment, Rhythm);
    assert_eq!(
        s.e.status().palette,
        Some(2),
        "one drop is one palette change"
    );
}

#[test]
fn a_confirmed_predicted_drop_does_not_swallow_the_next_drop() {
    use light_core::library::PaletteChange::AtDrop;
    let mut s = sim125();
    s.e.set_library(s.now, &two_pal_lib(AtDrop));
    s.start();
    s.beats_of(22, &mut |beat| match beat {
        4..=17 => Some(BuildUp),
        _ => Some(Rhythm),
    });
    assert_eq!(s.e.status().palette, Some(2));
    // A second build-up and drop: the palette changes again.
    s.beats_of(24, &mut |beat| match beat {
        22..=37 => Some(BuildUp),
        _ => Some(Rhythm),
    });
    assert_eq!(s.e.status().palette, Some(1));
}

#[test]
fn saving_the_library_while_stopped_does_not_paint_over_a_preview() {
    let mut s = sim125();
    let out = s.e.set_preview(s.now, Some(preview_pair()));
    s.record(s.now, out);
    s.deadlines_until(T0 + 600_000);
    let mark = s.sends.len();
    let out = s.e.set_library(s.now, &plain(4));
    s.record(s.now, out);
    s.deadlines_until(T0 + 700_000);
    let after = hues(&s.sends[mark..]);
    assert!(
        after.iter().all(|h| *h == 111 || *h == 222),
        "rest scene painted over the preview: {after:?}"
    );
}

#[test]
fn two_contradicted_predictions_and_the_real_drop_advance_the_palette_once() {
    use light_core::library::PaletteChange::AtDrop;
    let mut s = sim125();
    s.e.set_library(s.now, &two_pal_lib(AtDrop));
    s.start();
    // Predictions at the phrase starts (beats 15 and 31) are both contradicted; the drop is real at 44.
    s.beats_of(56, &mut |beat| match beat {
        4..=43 => Some(BuildUp),
        _ => Some(Rhythm),
    });
    assert_eq!(s.e.status().moment, Rhythm);
    assert_eq!(s.e.status().palette, Some(2));
}
