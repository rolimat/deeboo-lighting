import { expect, it } from "vitest";
import { createHistory } from "./history";

it("history_undo_redo_limit", () => {
  const h = createHistory(0, 3);
  expect([h.canUndo, h.canRedo]).toEqual([false, false]);
  for (const n of [1, 2, 3, 4, 5]) h.push(n);
  expect(h.present).toBe(5);
  h.undo();
  h.undo();
  h.undo();
  expect(h.present).toBe(2); // only 3 steps kept: 0 and 1 fell off
  expect(h.canUndo).toBe(false);
  h.redo();
  expect(h.present).toBe(3);
  h.push(9); // a new edit drops the redo branch
  expect(h.canRedo).toBe(false);
  expect(h.present).toBe(9);
});
