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
  cause: null,
  progress: {
    highBarDb: null,
    riserBars: 0,
    riserNeedBars: 4,
    riserRiseDb: 0,
    riserNeedDb: 6,
    approachCalmBars: 0,
    approachNeedBars: 4,
    approachAboveDb: 0,
    approachRiseDb: 0,
    approachNeedRiseDb: 2,
  },
});

it("ring_keeps_last_32_beats", () => {
  let ring: DetectorEvent[] = [];
  for (let i = 1; i <= 40; i++) ring = pushBeat(ring, ev(i));
  expect(RING_BEATS).toBe(32);
  expect(ring).toHaveLength(32);
  expect(ring[0]?.kickDb).toBe(-9);
  expect(ring[31]?.kickDb).toBe(-40);
});
