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
        let c = self.e.on_midi(t, status, || {
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
    // Beat 1 pulse at T0+480ms plans the step shown at T0+720ms; first message goes LEAD before.
    let at = T0 + 480_000 + 240_000 - LEAD;
    assert!(s.sends.iter().any(|(t, _)| *t == at), "{:?}", s.sends);
    let at2 = T0 + 960_000 + 240_000 - LEAD;
    assert!(s.sends.iter().any(|(t, _)| *t == at2), "{:?}", s.sends);
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
    assert!(
        matches!(&first[0].1, PaintCommand::Pencil { color, segments }
        if color.h == 0 && segments == &[1, 2]),
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
    // Rhythm step 0 (principal, principal, dark, dark), rebuilt on a strip in an unknown state.
    let got = apply_all(&s.sends_in(from, u64::MAX), vec![hsv(77, 77, 77); 4]);
    assert_eq!(
        got,
        vec![hsv(0, 1000, 1000), hsv(0, 1000, 1000), None, None]
    );
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
    s.ticks(70, &mut none);
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
    // Beat pulse 2 (half-pulse index 2) plans the step for T0 + 2250 ms.
    assert!(
        s.sends
            .iter()
            .any(|(t, _)| *t == T0 + 1_500_000 + 750_000 - 300_000)
    );

    let mut c = Sim::new(62_500);
    c.cmd(UiCommand::LeadMs(900));
    c.start();
    c.beats_of(2, &mut none);
    // Clamped to 500 ms: not sent at the pulse itself.
    assert!(c.sends.iter().all(|(t, _)| *t != T0 + 1_500_000));
    assert!(
        c.sends
            .iter()
            .any(|(t, _)| *t == T0 + 1_500_000 + 750_000 - 500_000)
    );
}
