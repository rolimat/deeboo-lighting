use light_core::clock::{ClockEvent, ClockTracker, PhraseBars, Position};
use proptest::prelude::*;

fn ticks(start_us: u64, interval_us: u64, n: u64) -> Vec<u64> {
    (0..n).map(|i| start_us + i * interval_us).collect()
}

/// Feeds 0xF8 at each time; returns the events produced.
fn feed(t: &mut ClockTracker, times: &[u64]) -> Vec<ClockEvent> {
    times.iter().filter_map(|&us| t.on_midi(us, 0xF8)).collect()
}

fn pos(e: ClockEvent) -> light_core::clock::Position {
    match e {
        ClockEvent::Pulse { pos, .. } => pos,
        ClockEvent::Stopped { .. } => unreachable!(),
    }
}

#[test]
fn first_tick_is_beat_one() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    let p = pos(t.on_midi(1_000, 0xF8).unwrap());
    assert_eq!(
        (p.beat, p.beat_in_bar, p.bar_in_phrase, p.half),
        (0, 1, 1, false)
    );
    assert!(p.is_bar_start() && p.is_phrase_start());
    assert!(t.running());
}

#[test]
fn start_arms_and_next_tick_is_beat_one() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    assert_eq!(t.on_midi(0, 0xFA), None);
    let p = pos(t.on_midi(10, 0xF8).unwrap());
    assert_eq!(p.beat, 0);
    t.on_midi(20, 0xFC);
    assert_eq!(t.on_midi(30, 0xFB), None);
    assert_eq!(pos(t.on_midi(40, 0xF8).unwrap()).beat, 0);
}

#[test]
fn start_while_running_resets_to_beat_one() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    feed(&mut t, &ticks(0, 20_000, 30));
    t.on_midi(600_000, 0xFA);
    assert_eq!(pos(t.on_midi(610_000, 0xF8).unwrap()).beat, 0);
}

#[test]
fn continue_after_stop_restarts_at_beat_one() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    feed(&mut t, &ticks(0, 20_000, 30));
    assert!(matches!(
        t.on_midi(700_000, 0xFC),
        Some(ClockEvent::Stopped { .. })
    ));
    assert_eq!(pos(t.on_midi(800_000, 0xF8).unwrap()).beat, 0);
}

#[test]
fn pulses_every_12_ticks_with_half_flag() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    let ev = feed(&mut t, &ticks(0, 20_000, 49));
    let ps: Vec<_> = ev.into_iter().map(pos).collect();
    assert_eq!(ps.len(), 5);
    assert_eq!(
        ps.iter().map(|p| (p.beat, p.half)).collect::<Vec<_>>(),
        [(0, false), (0, true), (1, false), (1, true), (2, false)]
    );
}

#[test]
fn bar_and_phrase_wrap_for_4_8_16() {
    for (bars, expect_bar_at_32) in [
        (PhraseBars::Four, 1),
        (PhraseBars::Eight, 1),
        (PhraseBars::Sixteen, 9),
    ] {
        let mut t = ClockTracker::new(bars);
        let ev = feed(&mut t, &ticks(0, 1_000, 32 * 24 + 1));
        let p = pos(*ev.last().unwrap());
        assert_eq!(p.beat, 32);
        assert_eq!(p.bar_in_phrase, expect_bar_at_32);
        assert_eq!(p.is_phrase_start(), bars != PhraseBars::Sixteen);
        assert_eq!(bars.bars() as usize, [4, 8, 16][bars as usize]);
    }
    let mut t = ClockTracker::new(PhraseBars::Four);
    let ev = feed(&mut t, &ticks(0, 1_000, 15 * 24 + 1));
    assert!(pos(*ev.last().unwrap()).is_last_beat_of_phrase());
}

#[test]
fn stop_byte_stops() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    feed(&mut t, &ticks(0, 20_000, 5));
    assert_eq!(
        t.on_midi(100_000, 0xFC),
        Some(ClockEvent::Stopped { at_us: 100_000 })
    );
    assert!(!t.running());
    assert_eq!(t.bpm(), None);
}

#[test]
fn stops_after_500ms_without_clock() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    let times = ticks(0, 20_000, 5);
    feed(&mut t, &times);
    let last = *times.last().unwrap();
    assert_eq!(t.next_deadline_us(), Some(last + 500_000));
    assert_eq!(t.on_deadline(last + 499_999), None);
    assert_eq!(
        t.on_deadline(last + 500_000),
        Some(ClockEvent::Stopped {
            at_us: last + 500_000
        })
    );
    assert_eq!(t.on_deadline(last + 600_000), None);
}

#[test]
fn bpm_125_from_20ms_ticks() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    assert_eq!(t.bpm(), None);
    feed(&mut t, &ticks(0, 20_000, 40));
    assert!((t.bpm().unwrap() - 125.0).abs() < 0.05);
    assert_eq!(t.half_period_us(), Some(240_000));
}

#[test]
fn bpm_follows_tempo_change() {
    let mut t = ClockTracker::new(PhraseBars::Eight);
    let a = ticks(0, 20_833, 24); // ~120
    feed(&mut t, &a);
    let start = a[23] + 19_531;
    feed(&mut t, &ticks(start, 19_531, 24)); // ~128
    assert!((t.bpm().unwrap() - 128.0).abs() < 0.1);
}

#[test]
fn next_deadline_none_when_stopped() {
    let t = ClockTracker::new(PhraseBars::Eight);
    assert_eq!(t.next_deadline_us(), None);
}

#[test]
fn position_after_is_next_half_beat() {
    let mut t = ClockTracker::new(PhraseBars::Four);
    let p = pos(t.on_midi(0, 0xF8).unwrap());
    let h = t.position_after(p);
    assert_eq!((h.beat, h.half), (0, true));
    let n = t.position_after(h);
    assert_eq!((n.beat, n.half, n.beat_in_bar), (1, false, 2));
}

proptest! {
    #[test]
    fn ignores_non_clock_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
        let mut t = ClockTracker::new(PhraseBars::Eight);
        for (i, b) in bytes.into_iter().enumerate() {
            if matches!(b, 0xF8 | 0xFA | 0xFB | 0xFC) { continue; }
            prop_assert_eq!(t.on_midi(i as u64 * 1000, b), None);
        }
        prop_assert!(!t.running());
    }
}

#[test]
fn previous_beat_of_phrase_start_is_last_beat_of_phrase() {
    let eight = PhraseBars::Eight;
    let prev = Position::at(32, false, eight).previous_beat();
    assert_eq!(prev, Some(Position::at(31, false, eight)));
    let prev = prev.unwrap();
    assert_eq!((prev.beat_in_bar, prev.bar_in_phrase), (4, 8));
    assert_eq!(Position::at(0, false, eight).previous_beat(), None);
    assert_eq!(
        Position::at(5, true, eight).previous_beat(),
        Some(Position::at(4, false, eight))
    );
}
