import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library, Piece } from "../api";

vi.mock("../api", () => ({
  api: { getLibrary: vi.fn(), saveLibrary: vi.fn(), identifySegment: vi.fn() },
}));
import { api } from "../api";
import { ShapeEditor } from "./ShapeEditor";

const row = (n: number): Piece[] =>
  Array.from({ length: n }, (_, i) => ({ x: i * 2, y: 0, angle: 0, len: 1 }));
const lib = (pieces: Piece[]) => ({ schemaVersion: 1, shape: { pieces } }) as unknown as Library;

let root: HTMLElement;
const settle = () => new Promise((r) => setTimeout(r));

async function mount(segments = 3) {
  await act(async () => {
    render(<ShapeEditor segments={segments} />, root);
    await settle();
  });
}
const piece = (n: number): SVGGElement => {
  const el = root.querySelector<SVGGElement>(`[aria-label='Segment ${n}']`);
  if (!el) throw new Error(`no segment ${n}`);
  return el;
};
const button = (label: string): HTMLButtonElement => {
  const b = [...root.querySelectorAll("button")].find((x) => x.textContent === label);
  if (!b) throw new Error(`no button ${label}`);
  return b;
};
const press = (el: Element, key: string) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
  });

beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
  vi.mocked(api.getLibrary).mockResolvedValue(lib(row(3)));
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  vi.mocked(api.identifySegment).mockResolvedValue(undefined);
});
afterEach(async () => {
  // act() flushes pending effects and cleanups before jsdom is torn down.
  await act(async () => render(null, root));
  root.remove();
  vi.clearAllMocks();
});

it("click_identifies_segment", async () => {
  await mount();
  expect(root.querySelectorAll("[data-piece]")).toHaveLength(3);
  await act(async () => {
    piece(2).dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  expect(api.identifySegment).toHaveBeenCalledWith(2);
  vi.mocked(api.identifySegment).mockRejectedValue("Strip not connected.");
  await act(async () => {
    piece(1).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await settle();
  });
  expect(root.querySelector("[role='alert']")?.textContent).toBe("Strip not connected.");
});

it("keyboard_moves_and_rotates", async () => {
  await mount();
  const p = piece(1);
  expect(p.getAttribute("tabindex")).toBe("0");
  await press(p, "ArrowRight");
  await press(p, "ArrowDown");
  await press(p, "r");
  await press(p, "ArrowLeft");
  await press(p, "ArrowLeft"); // clamped at the grid edge
  await act(async () => button("Save").click());
  const saved = vi.mocked(api.saveLibrary).mock.calls[0]?.[0].shape.pieces;
  expect(saved?.[0]).toEqual({ x: 0, y: 1, angle: 90, len: 1 });
  expect(saved?.[1]).toEqual(row(3)[1]);
});

it("length_buttons_apply_to_the_selected_piece_within_1_to_4", async () => {
  await mount();
  await act(async () => piece(2).focus());
  for (let i = 0; i < 5; i++) await act(async () => button("Longer").click());
  await act(async () => button("Save").click());
  expect(vi.mocked(api.saveLibrary).mock.calls[0]?.[0].shape.pieces[1]?.len).toBe(4);
});

it("save_sends_shape", async () => {
  const other = { ...lib(row(3)), activeSet: 7 } as unknown as Library;
  vi.mocked(api.getLibrary).mockResolvedValue(other);
  await mount();
  await act(async () => {
    button("Save").click();
    await settle();
  });
  expect(api.saveLibrary).toHaveBeenCalledWith({ ...other, shape: { pieces: row(3) } });
  expect(root.textContent).toContain("Saved.");
  vi.mocked(api.saveLibrary).mockRejectedValue("shape: too big");
  await act(async () => {
    button("Save").click();
    await settle();
  });
  expect(root.querySelector("[role='alert']")?.textContent).toBe("shape: too big");
});

it("uses_a_row_when_the_stored_shape_has_another_piece_count", async () => {
  vi.mocked(api.getLibrary).mockResolvedValue(lib(row(5)));
  await mount(3);
  expect(root.querySelectorAll("[data-piece]")).toHaveLength(3);
});
