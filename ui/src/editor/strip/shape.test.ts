import { expect, it } from "vitest";
import { bounds, hitTest, rotate, snap } from "./shape";
import type { Piece } from "../api";

const p = (x: number, y: number, angle: Piece["angle"], len: Piece["len"]): Piece => ({
  x,
  y,
  angle,
  len,
});

it("snap_and_rotate_helpers", () => {
  expect(snap(2.4)).toBe(2);
  expect(snap(2.5)).toBe(3);
  expect(snap(-3)).toBe(0);
  expect(snap(99)).toBe(64);
  expect(snap(Number.NaN)).toBe(0);
  expect([0, 90, 180, 270].map((a) => rotate(a as Piece["angle"]))).toEqual([90, 180, 270, 0]);
  // A 3-cell piece turned 90 degrees runs down from its anchor, one cell to the left.
  expect(bounds([p(5, 5, 90, 3)])).toEqual({ x0: 4, y0: 5, x1: 5, y1: 8 });
  expect(bounds([p(0, 0, 0, 2), p(1, 3, 180, 1)])).toEqual({ x0: 0, y0: 0, x1: 2, y1: 3 });
  const pieces = [p(0, 0, 0, 4), p(2, 0, 0, 1)];
  expect(hitTest(pieces, 0.5, 0.5)).toBe(0);
  expect(hitTest(pieces, 2.5, 0.5)).toBe(1); // topmost wins where they overlap
  expect(hitTest(pieces, 9, 9)).toBe(-1);
});
