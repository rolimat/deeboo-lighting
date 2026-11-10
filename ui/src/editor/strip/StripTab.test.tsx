import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library, Piece } from "../api";

vi.mock("../api", () => ({
  api: {
    getLibrary: vi.fn(),
    saveLibrary: vi.fn(),
    identifySegment: vi.fn(),
    setWiredSegments: vi.fn(),
  },
}));
import { api } from "../api";
import { useLibrary } from "../state/library";
import { StripTab } from "./StripTab";
import { Guide } from "./Guide";
import { preset } from "./presets";
import { endpoints } from "./shape";

const row = (n: number): Piece[] =>
  Array.from({ length: n }, (_, i) => ({ x: i, y: 0, angle: 0, len: 1 }));
const lib = (pieces: Piece[]): Library =>
  ({
    schemaVersion: 2,
    shape: { pieces },
    palettes: [],
    sequences: [],
    sets: [],
    activeSet: 1,
    activePalette: null,
  }) as Library;

let root: HTMLElement;
let store: ReturnType<typeof useLibrary>;
const onRepair = vi.fn();
const onWired = vi.fn();
const settle = (ms = 0) => new Promise((r) => setTimeout(r, ms));

function Harness({ wired = 8, reported = 10 }: { wired?: number; reported?: number }) {
  store = useLibrary();
  return (
    <StripTab
      store={store}
      strip={{ name: "dj lights", segments: wired, reported, ip: "192.168.1.42" }}
      onRepair={onRepair}
      onWired={onWired}
    />
  );
}
async function mount(n = 3, wired = n, reported = 10) {
  vi.mocked(api.getLibrary).mockResolvedValue(lib(row(n)));
  await act(async () => {
    render(<Harness wired={wired} reported={reported} />, root);
    await settle();
  });
}
const pieces = () => store.lib?.shape.pieces ?? [];
const q = <T extends Element>(sel: string): T => {
  const el = root.querySelector<T>(sel);
  if (!el) throw new Error(`nothing matches ${sel}`);
  return el;
};
const segment = (n: number) => q<SVGGElement>(`[aria-label='Segment ${n}']`);
const button = (label: string): HTMLButtonElement => {
  const b = [...root.querySelectorAll("button")].find(
    (x) => x.textContent === label || x.getAttribute("aria-label") === label,
  );
  if (!b) throw new Error(`no button ${label}`);
  return b;
};

// The canvas is 1400 x 700 px; this maps grid cells (in the current view) to client pixels.
const W = 1400;
const H = 700;
function at(cx: number, cy: number) {
  const svg = q<SVGSVGElement>("svg[aria-label='Strip shape']");
  const [vx = 0, vy = 0, vw = 1, vh = 1] = (svg.getAttribute("viewBox") ?? "")
    .split(" ")
    .map(Number);
  return { clientX: ((cx - vx) / vw) * W, clientY: ((cy - vy) / vh) * H };
}
const fire = (el: Element, type: string, cx: number, cy: number, extra: MouseEventInit = {}) =>
  act(() => {
    el.dispatchEvent(new MouseEvent(type, { bubbles: true, ...at(cx, cy), ...extra }));
  });
async function drag(el: Element, from: [number, number], to: [number, number], extra = {}) {
  await fire(el, "pointerdown", ...from, extra);
  await fire(el, "pointermove", ...to, extra);
  await fire(el, "pointerup", ...to, extra);
}
const press = (el: Element, key: string) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
  });

beforeEach(() => {
  localStorage.setItem("deeboo.stripGuide", "done");
  root = document.createElement("div");
  document.body.append(root);
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    right: W,
    bottom: H,
    width: W,
    height: H,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  vi.mocked(api.identifySegment).mockResolvedValue(undefined);
  vi.mocked(api.setWiredSegments).mockImplementation(async (n) => lib(row(n)));
});
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
  localStorage.clear();
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

it("drag_moves_in_all_directions_canvas_grows", async () => {
  await mount(3);
  // Piece 2 (x 1) is dragged 4 cells left and 2 up, past the shape's top-left corner.
  await drag(segment(2), [1.5, 0.5], [-2.5, -1.5]);
  // Everything moves together so nothing is negative; the dragged piece keeps its place relative to the rest.
  expect(pieces().map((p) => [p.x, p.y])).toEqual([
    [3, 2],
    [0, 0],
    [5, 2],
  ]);
  // The view follows the grown shape and the whole gesture is one undo step.
  await act(async () => store.undo());
  expect(pieces().map((p) => [p.x, p.y])).toEqual([
    [0, 0],
    [1, 0],
    [2, 0],
  ]);
  expect(store.canUndo).toBe(false);
});

it("drag_snaps_to_the_grid_and_to_a_neighbours_end", async () => {
  await mount(3);
  // Dropped a little past piece 3's end: starts exactly at (3, 0.5) -> anchor (3, 0).
  await drag(segment(1), [0.5, 0.5], [3.4, 0.7]);
  expect(pieces()[0]).toEqual({ x: 3, y: 0, angle: 0, len: 1 });
});

it("shift_drag_moves_the_whole_shape", async () => {
  await mount(3);
  await drag(segment(2), [1.5, 0.5], [3.5, 2.5], { shiftKey: true });
  expect(pieces().map((p) => [p.x, p.y])).toEqual([
    [2, 2],
    [3, 2],
    [4, 2],
  ]);
});

it("rotate_handle_and_15_degree_buttons", async () => {
  await mount(3);
  await fire(segment(1), "pointerdown", 0.5, 0.5);
  await fire(segment(1), "pointerup", 0.5, 0.5);
  await act(async () => button("Rotate +15°").click());
  expect(pieces()[0]?.angle).toBe(15);
  await act(async () => button("Rotate −15°").click());
  await act(async () => button("Rotate −15°").click());
  expect(pieces()[0]?.angle).toBe(345);
  expect(root.textContent).toContain("Segment 1 · 345°");
  // The handle at the tip snaps to 15 degrees: pointing straight down from the piece's start is 90.
  const before = endpoints(pieces()[0] as Piece).a;
  const handle = q<SVGElement>("[aria-label='Rotate segment 1']");
  await fire(handle, "pointerdown", before.x + 1, before.y);
  await fire(handle, "pointermove", before.x + 0.1, before.y + 3);
  await fire(handle, "pointerup", before.x + 0.1, before.y + 3);
  const p = pieces()[0];
  expect(p?.angle).toBe(90);
  // It turned about its start: that point did not move.
  const after = endpoints(p as Piece).a;
  expect(after.x).toBeCloseTo(before.x, 3);
  expect(after.y).toBeCloseTo(before.y, 3);
});

it("arrow_keys_move_selected", async () => {
  await mount(3);
  await press(segment(3), "ArrowRight");
  expect(pieces()[2]?.x).toBe(3);
  await press(segment(3), "ArrowUp");
  // Going above the top moves the rest down instead: nothing is ever negative.
  expect(pieces().map((p) => p.y)).toEqual([1, 1, 0]);
  await press(segment(1), "ArrowLeft");
  expect(pieces().map((p) => p.x)).toEqual([0, 2, 4]);
});

it("selecting_piece_identifies_segment", async () => {
  await mount(3);
  await fire(segment(1), "pointerdown", 0.5, 0.5);
  await fire(segment(2), "pointerdown", 1.5, 0.5);
  expect(api.identifySegment).not.toHaveBeenCalled();
  await act(async () => void (await settle(300)));
  expect(api.identifySegment).toHaveBeenCalledTimes(1);
  expect(api.identifySegment).toHaveBeenCalledWith(2);
  expect(root.textContent).toContain("Segment 2 · 0°");
});

it("segments_stepper_greys_unwired", async () => {
  await mount(8, 8, 10);
  const nums = [...root.querySelectorAll<HTMLElement>("[data-segment-number]")];
  expect(nums.map((n) => n.textContent)).toEqual(Array.from({ length: 10 }, (_, i) => `${i + 1}`));
  expect(nums.map((n) => n.dataset.wired)).toEqual([...Array(8).fill("true"), "false", "false"]);
  expect(root.textContent).toContain(
    "The controller reports 10; only 8 are wired. Click a number to light that segment and count.",
  );
  await act(async () => nums[8]?.click());
  expect(api.identifySegment).toHaveBeenCalledWith(9);
  await act(async () => {
    button("More segments").click();
    await settle();
  });
  expect(api.setWiredSegments).toHaveBeenCalledWith(9);
  expect(onWired).toHaveBeenCalled();
  expect(pieces()).toHaveLength(9);
  expect(store.canUndo).toBe(false);
});

it("reducing_segments_asks_first", async () => {
  await mount(8, 8, 10);
  await act(async () => button("Fewer segments").click());
  expect(root.textContent).toContain("Reduce to 7 segments? Sequences lose segments 8–8.");
  expect(api.setWiredSegments).not.toHaveBeenCalled();
  await act(async () => button("Cancel").click());
  await act(async () => button("Fewer segments").click());
  await act(async () => {
    button("Reduce").click();
    await settle();
  });
  expect(api.setWiredSegments).toHaveBeenCalledWith(7);
});

it("presets_replace_the_shape_in_one_undo_step", async () => {
  await mount(8, 8, 10);
  await act(async () => button("U shape").click());
  expect(pieces()).toEqual(preset("uShape", 8));
  await act(async () => store.undo());
  expect(pieces()).toEqual(row(8));
});

it("header_card_has_status_and_both_reconnect_buttons", async () => {
  await mount(8, 8, 10);
  expect(root.textContent).toContain("dj lights");
  expect(root.textContent).toContain("Connected · 192.168.1.42");
  await act(async () => button("Reconnect…").click());
  await act(async () => button("Change strip…").click());
  expect(onRepair).toHaveBeenCalledTimes(2);
  expect(root.textContent).toContain(
    "Arrows move the selected piece · ⇧ + drag moves the whole shape · ⌘Z undo",
  );
});

it("guide_shows_first_time_and_with_help", async () => {
  const g = (helpOn: boolean) => async () =>
    act(async () => {
      render(<Guide helpOn={helpOn} />, root);
      await settle();
    });
  localStorage.clear();
  await g(false)();
  expect(root.textContent).toContain("How to draw your strip");
  expect(root.textContent).not.toMatch(/video|demo/i);
  await act(async () => button("Got it").click());
  expect(root.textContent).not.toContain("How to draw your strip");
  expect(localStorage.getItem("deeboo.stripGuide")).toBe("done");
  // Dismissed: still hidden with help off, back when help is switched on.
  await g(false)();
  expect(root.textContent).not.toContain("How to draw your strip");
  await g(true)();
  expect(root.textContent).toContain("How to draw your strip");
});

it("guide_visible_whenever_help_is_on", async () => {
  const g = (helpOn: boolean) =>
    act(async () => {
      render(<Guide helpOn={helpOn} />, root);
      await settle();
    });
  localStorage.setItem("deeboo.stripGuide", "done");
  await g(false);
  expect(root.textContent).not.toContain("How to draw your strip");
  await g(true);
  expect(root.textContent).toContain("How to draw your strip");
  await act(async () => button("Got it").click());
  expect(root.textContent).not.toContain("How to draw your strip");
  await g(false);
  await g(true); // help toggled off and on again
  expect(root.textContent).toContain("How to draw your strip");
  // Opened with help already on: shown even though it was dismissed before.
  await act(async () => render(null, root));
  await g(true);
  expect(root.textContent).toContain("How to draw your strip");
});
