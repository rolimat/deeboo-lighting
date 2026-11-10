import { expect, it } from "vitest";
import type { Step } from "../api";
import { fromCells, setBars, toCells } from "./cells";

const HB: Record<string, number> = { half: 1, one: 2, two: 4, four: 8 };
const ROLES = ["principal", "contraste", "suave", "off"];

/** Small deterministic PRNG so the property-style cases are repeatable. */
function rng(seed: number) {
  let s = seed;
  return () => (s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32;
}
function randomCols(r: () => number, bars: number, segs: number): string[][] {
  const cols: string[][] = [];
  while (cols.length < bars * 8) {
    // Runs of equal columns, so merging has something to do.
    const col = Array.from({ length: segs }, () => ROLES[Math.floor(r() * ROLES.length)] as string);
    for (let n = 1 + Math.floor(r() * 5); n > 0 && cols.length < bars * 8; n--) cols.push(col);
  }
  return cols;
}
const halfBeats = (steps: Step[]) => steps.reduce((a, s) => a + (HB[s.len] ?? 0), 0);

it("cells_round_trip", () => {
  const r = rng(7);
  for (let i = 0; i < 200; i++) {
    const cols = randomCols(r, 1 + Math.floor(r() * 16), 1 + Math.floor(r() * 8));
    const steps = fromCells(cols);
    expect(toCells(steps)).toEqual(cols);
    expect(halfBeats(steps)).toBe(cols.length);
    // and back from steps: cells are stable
    expect(toCells(fromCells(toCells(steps)))).toEqual(cols);
  }
});

it("steps_never_cross_bars", () => {
  const r = rng(11);
  for (let i = 0; i < 200; i++) {
    const steps = fromCells(randomCols(r, 1 + Math.floor(r() * 16), 3));
    let pos = 0;
    for (const s of steps) {
      const len = HB[s.len] ?? 0;
      expect(pos % 8).toBeLessThanOrEqual(8 - len);
      pos += len;
    }
  }
});

it("equal_columns_merge_into_the_longest_step_that_fits", () => {
  const same = Array.from({ length: 16 }, () => ["principal"]);
  expect(fromCells(same).map((s) => s.len)).toEqual(["four", "four"]);
  const five = Array.from({ length: 8 }, (_, i) => (i < 5 ? ["suave"] : ["off"]));
  expect(fromCells(five).map((s) => s.len)).toEqual(["two", "half", "one", "half"]);
});

it("drag_edge_adds_whole_bar_copying_last", () => {
  const a = ["principal"];
  const b = ["contraste"];
  const cols = [...Array(8).fill(a), ...Array(8).fill(b)];
  const grown = setBars(cols, 3);
  expect(grown).toHaveLength(24);
  expect(grown.slice(16)).toEqual(cols.slice(8));
  expect(setBars(grown, 1)).toEqual(cols.slice(0, 8));
  expect(setBars(cols, 0)).toHaveLength(8); // at least one bar
  expect(setBars(cols, 99)).toHaveLength(16 * 8); // at most 16 bars
});
