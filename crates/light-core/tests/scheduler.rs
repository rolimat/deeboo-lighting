use light_core::model::Colour;
use light_core::paint::{Hsv, PaintCommand};
use light_core::scheduler::{MESSAGE_GAP_US, Scheduler};

const fn c(h: u16) -> Hsv {
    Hsv { h, s: 1000, v: 500 }
}

/// Three distinct colours on 3 segments: diff against dark is 3 pencils.
fn three(a: u16) -> Vec<Colour> {
    vec![Some(c(a)), Some(c(a + 1)), Some(c(a + 2))]
}

#[test]
fn messages_start_lead_before_the_step_and_are_spaced() {
    let mut s = Scheduler::new(3);
    s.plan(0, 1_000_000, 200_000, &three(10));
    assert_eq!(s.next_deadline_us(), Some(800_000));
    assert_eq!(s.due(799_999), Vec::<PaintCommand>::new());
    assert_eq!(s.due(800_000).len(), 1);
    assert_eq!(s.next_deadline_us(), Some(800_000 + MESSAGE_GAP_US));
    assert_eq!(s.due(800_000 + 2 * MESSAGE_GAP_US).len(), 2);
    assert_eq!(s.next_deadline_us(), None);
}

#[test]
fn gap_holds_across_steps() {
    let mut s = Scheduler::new(3);
    s.plan(0, 0, 0, &[Some(c(1)); 3]);
    assert_eq!(s.due(0).len(), 1);
    s.plan(10_000, 10_000, 0, &[Some(c(2)); 3]);
    assert_eq!(s.next_deadline_us(), Some(MESSAGE_GAP_US));
}

#[test]
fn unsent_messages_are_dropped_when_the_next_step_is_planned() {
    let mut s = Scheduler::new(3);
    let a = three(10);
    s.plan(0, 0, 0, &a);
    let first = s.due(0);
    assert_eq!(first.len(), 1);
    let b = three(50);
    s.plan(1, 1, 0, &b);
    let mut expect = vec![None; 3];
    light_core::scene::apply(&mut expect, first.first().unwrap());
    assert_eq!(s.sent(), expect.as_slice());
    let cmds = s.due(10_000_000);
    let mut state = expect.clone();
    for cmd in &cmds {
        light_core::scene::apply(&mut state, cmd);
    }
    assert_eq!(state, b);
    assert_eq!(cmds, light_core::scene::diff(&expect, &b));
}

#[test]
fn late_plan_sends_from_now() {
    let mut s = Scheduler::new(3);
    s.plan(5_000_000, 1_000, 500_000, &three(10));
    assert_eq!(s.next_deadline_us(), Some(5_000_000));
}

#[test]
fn no_deadline_when_nothing_pending() {
    let mut s = Scheduler::new(3);
    assert_eq!(s.next_deadline_us(), None);
    s.plan(0, 0, 0, &[None; 3]);
    assert_eq!(s.next_deadline_us(), None);
    assert_eq!(s.due(0), Vec::<PaintCommand>::new());
}

#[test]
fn forget_sent_makes_the_next_plan_a_full_scene() {
    let mut s = Scheduler::new(3);
    s.plan(0, 0, 0, &[Some(c(1)); 3]);
    s.due(0);
    s.plan(1_000_000, 1_000_000, 0, &[Some(c(1)); 3]);
    assert_eq!(s.next_deadline_us(), None);
    s.forget_sent();
    s.plan(1_000_000, 1_000_000, 0, &[Some(c(1)); 3]);
    assert_eq!(s.due(1_000_000), vec![PaintCommand::Bucket(c(1))]);
}
