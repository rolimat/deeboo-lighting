import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library } from "../api";

vi.mock("../api", () => ({
  api: { getLibrary: vi.fn(), saveLibrary: vi.fn() },
}));
import { api } from "../api";
import { useLibrary } from "./library";

const lib0: Library = {
  schemaVersion: 2,
  shape: { pieces: [] },
  palettes: [],
  sequences: [],
  sets: [],
  activeSet: 1,
  activePalette: null,
};
const piece = (x: number) => ({ x, y: 0, angle: 0 as const, len: 1 as const });

let h: ReturnType<typeof useLibrary>;
function Probe() {
  h = useLibrary();
  return null;
}
const tick = (ms: number) => act(async () => void (await vi.advanceTimersByTimeAsync(ms)));

beforeEach(async () => {
  vi.useFakeTimers();
  vi.mocked(api.getLibrary).mockResolvedValue(lib0);
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  await act(async () => render(<Probe />, document.createElement("div")));
  await tick(0);
});
afterEach(() => {
  vi.useRealTimers();
  vi.resetAllMocks();
});

const addPiece = (x: number) => () =>
  h.edit((l) => ({ ...l, shape: { pieces: [...l.shape.pieces, piece(x)] } }));

it("edits_debounce_into_one_save", async () => {
  for (const x of [1, 2, 3]) {
    await act(async () => addPiece(x)());
    await tick(100);
  }
  expect(h.save.state).toBe("saving");
  await tick(299);
  expect(api.saveLibrary).not.toHaveBeenCalled();
  await tick(1);
  expect(api.saveLibrary).toHaveBeenCalledTimes(1);
  expect(vi.mocked(api.saveLibrary).mock.calls[0]?.[0].shape.pieces).toHaveLength(3);
  expect(h.save.state).toBe("saved");
});

it("drag_gesture_is_one_undo_step", async () => {
  for (const x of [1, 2, 3, 4, 5]) {
    await act(async () =>
      h.edit((l) => ({ ...l, shape: { pieces: [piece(x)] } }), { coalesce: "drag" }),
    );
    await tick(20);
  }
  expect(h.lib?.shape.pieces[0]?.x).toBe(5);
  await act(async () => h.undo());
  expect(h.lib?.shape.pieces).toHaveLength(0);
  expect(h.canUndo).toBe(false);
  expect(h.canRedo).toBe(true);
});

it("undo_never_sends_selection", async () => {
  // The live set changed (play button) while editing: saves answer with the live selection.
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => ({ ...l, activeSet: 5 }));
  await act(async () => addPiece(1)());
  await tick(400);
  await act(async () => addPiece(2)());
  await tick(400);
  await act(async () => h.undo());
  await tick(400);
  const last = vi.mocked(api.saveLibrary).mock.calls.at(-1)?.[0];
  expect(last?.shape.pieces).toHaveLength(1);
  expect(last?.activeSet).toBe(5);
});

it("rejected_save_keeps_state", async () => {
  vi.mocked(api.saveLibrary).mockRejectedValueOnce("sets references an unknown palette");
  await act(async () => addPiece(1)());
  await tick(400);
  expect(h.save).toEqual({ state: "error", reason: "sets references an unknown palette" });
  expect(h.lib?.shape.pieces).toHaveLength(1);
  await act(async () => addPiece(2)());
  await tick(400);
  expect(h.save.state).toBe("saved");
  expect(vi.mocked(api.saveLibrary).mock.calls.at(-1)?.[0].shape.pieces).toHaveLength(2);
});

it("reset_adopts_a_library_and_clears_undo", async () => {
  await act(async () => addPiece(1)());
  expect(h.canUndo).toBe(true);
  await act(async () => h.reset({ ...lib0, shape: { pieces: [piece(7), piece(8)] } }));
  expect(h.lib?.shape.pieces).toHaveLength(2);
  expect(h.canUndo).toBe(false);
  await tick(500);
  expect(api.saveLibrary).not.toHaveBeenCalled();
  expect(h.save.state).toBe("saved");
});
