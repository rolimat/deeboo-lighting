import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library, Sequence } from "../api";

vi.mock("../api", () => ({
  api: { getLibrary: vi.fn(), saveLibrary: vi.fn(), messageCounts: vi.fn() },
}));
import { api } from "../api";
import { SequenceList } from "./SequenceList";

const hsv = (h: number) => ({ h, s: 1000, v: 1000 });
const lib = (): Library => ({
  schemaVersion: 2,
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
async function mount(stripSegments?: number) {
  await act(async () => {
    render(<SequenceList stripSegments={stripSegments} />, root);
    await settle();
  });
  // The library arrives after the first act, so its render (and the counts effect) flush at the
  // end of a second one; the debounced counts call then needs a window of its own.
  await act(async () => {
    await settle();
  });
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
afterEach(async () => {
  // act() flushes pending effects and cleanups before jsdom is torn down.
  await act(async () => render(null, root));
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
  await act(async () => {
    button("Save").click();
    await settle();
  });
  expect(root.textContent).toContain("sets references an unknown sequence");
});

const seqSet = (id: number, name: string) => {
  const ms = {
    sequences: [{ id }],
    order: "inOrder" as const,
    interval: { bars: 2 },
  };
  return {
    id: 1,
    name,
    rhythm: ms,
    calm: ms,
    buildup: ms,
    palettes: [1],
    paletteOrder: "inOrder" as const,
    paletteChange: "atDrop" as const,
  };
};

it("save_replaces_only_sequences_on_a_fresh_library", async () => {
  // The popover changed the active set after this section loaded.
  vi.mocked(api.getLibrary)
    .mockResolvedValueOnce(lib())
    .mockResolvedValueOnce({ ...lib(), activeSet: 5, activePalette: null });
  await mount();
  await act(async () => button("Save").click());
  const sent = vi.mocked(api.saveLibrary).mock.calls[0]?.[0];
  expect(sent?.activeSet).toBe(5);
  expect(sent?.activePalette).toBeNull();
  expect(sent?.sequences[0]?.[0]).toBe(7);
});

it("delete_is_disabled_while_a_set_uses_the_sequence", async () => {
  const l = lib();
  l.sequences.push([8, { ...(l.sequences[0] as [number, Sequence])[1], name: "Spare" }]);
  l.sets = [seqSet(7, "Club")];
  vi.mocked(api.getLibrary).mockResolvedValue(l);
  await mount();
  expect(button("Delete sequence").hasAttribute("disabled")).toBe(true);
  expect(root.textContent).toContain("Used by set Club \u2014 remove it there first.");
  await act(async () => button("Spare").click());
  expect(button("Delete sequence").hasAttribute("disabled")).toBe(false);
  expect(root.textContent).not.toContain("Used by set");
});

it("notes_when_the_sequences_are_for_another_segment_count", async () => {
  await mount(5);
  expect(root.textContent).toContain(
    "These sequences are for 3 segments; the strip has 5. Save the strip shape to adapt them.",
  );
  render(null, root);
  await mount(3);
  expect(root.textContent).not.toContain("These sequences are for");
});
