import { expect, it } from "vitest";
import { stepAt } from "./player";
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
