import { expect, it } from "vitest";
import { frameAt, stepAt } from "./player";
import fixture from "./player-fixture.json";

// Generated from the Rust engine by crates/light-core/tests/player_fixture.rs.
it("player_matches_engine_fixture", () => {
  expect(fixture.length).toBeGreaterThan(0);
  for (const c of fixture) {
    const seq = { steps: c.steps.map((len) => ({ len })) };
    for (const [half, idx] of c.shown as [number, number][]) {
      expect(stepAt(seq, half), `${c.steps} @${half}`).toBe(idx);
    }
  }
});

it("step_at_is_safe_on_empty_and_unknown", () => {
  expect(stepAt({ steps: [] }, 5)).toBe(0);
  expect(stepAt({ steps: [{ len: "?" }] }, 5)).toBe(0);
});

it("frame_at_matches_engine_fixture", () => {
  for (const c of fixture) {
    const seq = { moment: c.moment, fade: c.fade, steps: c.steps.map((len) => ({ len })) };
    for (const [half, idx, t] of c.shown as [number, number, number][]) {
      const f = frameAt(seq, half);
      expect({ idx: f.idx, t: f.t }, `${c.steps} ${c.moment} @${half}`).toEqual({ idx, t });
    }
  }
});

it("frame_at_cuts_the_first_pass_and_without_fade", () => {
  const steps = [{ len: "four" }, { len: "four" }];
  expect(frameAt({ moment: "calm", fade: true, steps }, 0).from).toBeNull();
  expect(frameAt({ moment: "calm", fade: true, steps }, 8).from).toBe(0);
  expect(frameAt({ moment: "calm", fade: true, steps }, 16).from).toBe(1);
  expect(frameAt({ moment: "calm", fade: false, steps }, 8).from).toBeNull();
  expect(frameAt({ moment: "rhythm", fade: true, steps }, 8).from).toBeNull();
});
