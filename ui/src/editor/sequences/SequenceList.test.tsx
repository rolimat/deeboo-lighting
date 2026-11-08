import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library } from "../api";

vi.mock("../api", () => ({
  api: { getLibrary: vi.fn(), saveLibrary: vi.fn(), messageCounts: vi.fn() },
}));
import { api } from "../api";
import { SequenceList } from "./SequenceList";

const hsv = (h: number) => ({ h, s: 1000, v: 1000 });
const lib = (): Library => ({
  schemaVersion: 1,
  shape: { pieces: Array.from({ length: 3 }, (_, i) => ({ x: i * 2, y: 0, angle: 0, len: 1 })) },
  palettes: [[1, { name: "Test", principal: hsv(0), contraste: hsv(240), suave: hsv(120) }]],
  sequences: [
    [
      7,
      {
        name: "Pulse",
        moment: "rhythm",
        steps: [
          { len: "one", roles: ["principal", "off", "off"] },
          { len: "two", roles: ["off", "off", "off"] },
        ],
      },
    ],
  ],
  sets: [],
  activeSet: 1,
  activePalette: 1,
});

let root: HTMLElement;
const settle = (ms = 0) => new Promise((r) => setTimeout(r, ms));
async function mount() {
  await act(async () => {
    render(<SequenceList />, root);
    await settle();
  });
  // Effects flush when act ends; the debounced counts call then needs its own window.
  await act(async () => {
    await settle(300);
  });
}
const q = <T extends Element>(sel: string): T => {
  const el = root.querySelector<T>(sel);
  if (!el) throw new Error(`no ${sel}`);
  return el;
};
const cell = (seg: number, step: number) =>
  q<HTMLButtonElement>(`button[aria-label^='Segment ${seg}, step ${step}:']`);
const button = (label: string) => {
  const b = [...root.querySelectorAll("button")].find((x) => x.textContent === label);
  if (!b) throw new Error(`no button ${label}`);
  return b;
};
const fire = (el: Element, type: string) =>
  act(() => {
    el.dispatchEvent(new Event(type, { bubbles: true }));
  });
const saved = () => vi.mocked(api.saveLibrary).mock.calls[0]?.[0].sequences[0]?.[1];

beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
  vi.mocked(api.getLibrary).mockResolvedValue(lib());
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  vi.mocked(api.messageCounts).mockResolvedValue([1, 1]);
});
afterEach(() => {
  render(null, root);
  root.remove();
  vi.clearAllMocks();
});

it("paint_cells_by_drag", async () => {
  await mount();
  await fire(button("Contraste"), "click");
  await fire(cell(1, 2), "pointerdown");
  await fire(cell(2, 2), "pointerenter");
  await fire(cell(3, 2), "pointerenter");
  await fire(document.body, "pointerup");
  await fire(cell(1, 1), "pointerenter"); // after release: no paint
  await act(async () => button("Save").click());
  expect(saved()?.steps[1]?.roles).toEqual(["contraste", "contraste", "contraste"]);
  expect(saved()?.steps[0]?.roles).toEqual(["principal", "off", "off"]);
});

it("warning_on_multi_message_columns", async () => {
  vi.mocked(api.messageCounts).mockResolvedValue([1, 3]);
  await mount();
  expect(root.textContent).toContain("Shown in 3 parts");
  expect(root.textContent).not.toContain("Shown in 1 part");
});

it("step_length_selector", async () => {
  await mount();
  const sel = q<HTMLSelectElement>("select[aria-label='Step 2 length']");
  sel.value = "four";
  await fire(sel, "change");
  await act(async () => button("Save").click());
  expect(saved()?.steps.map((s) => s.len)).toEqual(["one", "four"]);
});

it("rename_validation_message", async () => {
  await mount();
  const name = q<HTMLInputElement>("input[aria-label='Sequence name']");
  for (const bad of ["", "x".repeat(64)]) {
    name.value = bad;
    await fire(name, "input");
    expect(root.textContent).toContain("Name must be 1 to 63 characters");
    await act(async () => button("Save").click());
  }
  expect(api.saveLibrary).not.toHaveBeenCalled();
});

it("shows_the_save_error", async () => {
  vi.mocked(api.saveLibrary).mockRejectedValue("sets references an unknown sequence");
  await mount();
  await act(async () => button("Save").click());
  expect(root.textContent).toContain("sets references an unknown sequence");
});
