import { expect, it } from "vitest";
import { bounds, endpoints, fits, magnet, normalise, snap, spin, turn } from "./shape";
import type { Piece } from "../api";

const p = (x: number, y: number, angle = 0, len = 1): Piece => ({ x, y, angle, len });
const near = (a: number, b: number) => expect(a).toBeCloseTo(b, 6);

it("snap_and_turn_helpers", () => {
  expect(snap(2.4)).toBe(2);
  expect(snap(2.5)).toBe(3);
  expect(snap(-3.2)).toBe(-3);
  expect(snap(Number.NaN)).toBe(0);
  expect([turn(0, 15), turn(0, -15), turn(350, 15), turn(90, -105)]).toEqual([15, 345, 5, 345]);
});

it("endpoints_run_along_the_centre_line", () => {
  expect(endpoints(p(2, 3, 0, 2))).toEqual({ a: { x: 2, y: 3.5 }, b: { x: 4, y: 3.5 } });
  const q = endpoints(p(5, 5, 90, 3)); // turned clockwise: runs down, half a cell to the left
  near(q.a.x, 4.5);
  near(q.b.y, 8);
});

it("bounds_cover_rotated_pieces", () => {
  expect(bounds([p(5, 5, 90, 3)])).toEqual({ x0: 4, y0: 5, x1: 5, y1: 8 });
  expect(bounds([p(0, 0, 0, 2), p(1, 3, 180, 1)])).toEqual({ x0: 0, y0: 0, x1: 2, y1: 3 });
  const b = bounds([p(0, 0, 45, 1)]);
  near(b.x0, -Math.SQRT1_2);
  near(b.x1, Math.SQRT1_2);
});

it("normalise_moves_everything_in_whole_cells_so_nothing_is_negative", () => {
  const ps = [p(-3, 2), p(1, -2.5)];
  expect(normalise(ps)).toEqual([p(0, 5), p(4, 0.5)]);
  const ok = [p(0, 0), p(2, 1)];
  expect(normalise(ok)).toBe(ok);
  expect(fits(ok)).toBe(true);
  expect(fits([p(65, 0)])).toBe(false);
});

it("magnet_snaps_an_end_to_a_neighbours_end_within_a_small_radius", () => {
  const ps = [p(0, 0), p(5, 3)];
  // Dropped a little off the end of piece 1: it starts exactly where piece 1 ends.
  const a = magnet(ps, 1, 1.3, 0.2);
  near(a.x, 1);
  near(a.y, 0);
  // A turned piece hooks on by its end too.
  const m = magnet([p(0, 0), p(1, 1, 180)], 1, 2.3, 1.2);
  near(m.x, 2);
  near(m.y, 1);
  // Far away: plain grid.
  expect(magnet(ps, 1, 7, 7)).toEqual({ x: 7, y: 7 });
});

it("spin_turns_about_the_start_of_the_piece", () => {
  const s = spin(p(3, 3, 0, 2), 90);
  expect(s.angle).toBe(90);
  near(endpoints(s).a.x, 3);
  near(endpoints(s).a.y, 3.5);
});
