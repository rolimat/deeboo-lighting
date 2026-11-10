import { expect, it } from "vitest";
import type { DetectorEvent } from "../api";
import { RING_BEATS, pushBeat } from "./history";

const ev = (n: number): DetectorEvent => ({
  kickDb: -n,
  subDb: -n,
  highDb: -n,
  kickRefDb: -n,
  subRefDb: -n,
  highRefDb: -n,
  moment: "rhythm",
});

it("ring_keeps_last_32_beats", () => {
  let ring: DetectorEvent[] = [];
  for (let i = 1; i <= 40; i++) ring = pushBeat(ring, ev(i));
  expect(RING_BEATS).toBe(32);
  expect(ring).toHaveLength(32);
  expect(ring[0]?.kickDb).toBe(-9);
  expect(ring[31]?.kickDb).toBe(-40);
});
