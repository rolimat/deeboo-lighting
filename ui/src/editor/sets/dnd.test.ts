import { expect, it } from "vitest";
import { canDrop, moveRef, type DragItem } from "./dnd";

const seq = (moment: "rhythm" | "calm" | "buildUp"): DragItem => ({
  kind: "sequence",
  ref: { id: 1 },
  moment,
});

it("calm_sequence_cannot_drop_on_rhythm", () => {
  expect(canDrop(seq("calm"), { kind: "track", moment: "rhythm" })).toBe(false);
  expect(canDrop(seq("calm"), { kind: "track", moment: "buildUp" })).toBe(false);
  expect(canDrop(seq("calm"), { kind: "track", moment: "calm" })).toBe(true);
  expect(canDrop(seq("calm"), { kind: "palettes" })).toBe(false);
});

it("palettes_only_drop_on_the_palette_row", () => {
  const p: DragItem = { kind: "palette", id: 3 };
  expect(canDrop(p, { kind: "palettes" })).toBe(true);
  expect(canDrop(p, { kind: "track", moment: "calm" })).toBe(false);
});

it("clips_only_reorder_within_their_own_track", () => {
  const c: DragItem = { kind: "clip", from: "calm", index: 0 };
  expect(canDrop(c, { kind: "track", moment: "calm" })).toBe(true);
  expect(canDrop(c, { kind: "track", moment: "rhythm" })).toBe(false);
  expect(canDrop(c, { kind: "palettes" })).toBe(false);
});

it("move_ref_puts_the_item_before_the_target_and_end_appends", () => {
  expect(moveRef([1, 2, 3], 2, 0)).toEqual([3, 1, 2]);
  expect(moveRef([1, 2, 3], 0, 3)).toEqual([2, 3, 1]);
  expect(moveRef([1, 2, 3], 0, 2)).toEqual([2, 1, 3]);
});
