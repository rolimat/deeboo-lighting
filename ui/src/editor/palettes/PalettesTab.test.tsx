import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library } from "../api";

vi.mock("../api", () => ({
  api: {
    getLibrary: vi.fn(),
    saveLibrary: vi.fn(),
    previewPalette: vi.fn(),
    getFullStatus: vi.fn(async () => ({ bpm: null, playing: false })),
    onStatus: vi.fn(async () => () => {}),
  },
}));
import { api } from "../api";
import { useLibrary } from "../state/library";
import { renumbering } from "../state/renumber-mock";
import { PalettesTab } from "./PalettesTab";
import type { BarActions } from "../sequences/SequencesTab";

const hsv = (h: number, s = 1000, v = 1000) => ({ h, s, v });
const lib = (): Library => ({
  schemaVersion: 3,
  shape: { pieces: Array.from({ length: 3 }, (_, i) => ({ x: i * 2, y: 0, angle: 0, len: 1 })) },
  palettes: [
    [1, { name: "Neon", principal: hsv(300), contraste: hsv(180), suave: hsv(260, 600, 700) }],
    [2, { name: "Sunset", principal: hsv(20), contraste: hsv(340), suave: hsv(45, 700, 800) }],
    [3, { name: "Ice", principal: hsv(200), contraste: hsv(0, 0), suave: hsv(220, 500, 600) }],
  ],
  sequences: [
    [
      7,
      {
        name: "Pulse",
        moment: "rhythm",
        fade: true,
        steps: [{ len: "one", roles: ["principal", "contraste", "suave"] }],
      },
    ],
  ],
  sets: [
    {
      id: 1,
      name: "Club night",
      rhythm: { sequences: [{ id: 7 }], order: "inOrder", interval: { bars: 4 } },
      calm: { sequences: ["generated"], order: "inOrder", interval: { bars: 4 } },
      buildup: { sequences: ["generated"], order: "inOrder", interval: { bars: 4 } },
      palettes: [1, 2],
      paletteOrder: "inOrder",
      paletteChange: "atDrop",
    },
  ],
  activeSet: 1,
  activePalette: null,
});

let root: HTMLElement;
let store: ReturnType<typeof useLibrary>;
let actions: (BarActions & { deleteGuard?: () => boolean }) | null = null;
const goSets = vi.fn();
const settle = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function Harness() {
  store = useLibrary();
  return (
    <PalettesTab
      store={store}
      onGoSets={goSets}
      onActions={(a) => {
        actions = a;
      }}
    />
  );
}
async function mount() {
  await act(async () => {
    render(<Harness />, root);
    await settle();
  });
  await act(async () => {
    await settle(10);
  });
}
const tick = (ms = 0) => act(async () => void (await settle(ms)));
const click = (el: Element | null | undefined) => act(() => void (el as HTMLElement).click());
const q = <T extends Element = HTMLElement>(sel: string) => root.querySelector<T>(sel);
const pal = (id: number) => store.lib?.palettes.find(([i]) => i === id)?.[1];
const dt = () => {
  const data = new Map<string, string>();
  return {
    setData: (k: string, v: string) => data.set(k, v),
    getData: (k: string) => data.get(k) ?? "",
  };
};
const fire = (el: Element | null, type: string, extra: object = {}) =>
  act(
    () =>
      void el?.dispatchEvent(
        Object.assign(new Event(type, { bubbles: true, cancelable: true }), extra),
      ),
  );
const pointer = (el: Element | null, type: string, x: number, y: number) =>
  act(
    () => void el?.dispatchEvent(new MouseEvent(type, { bubbles: true, clientX: x, clientY: y })),
  );
const typeInto = (el: HTMLInputElement | null, value: string) =>
  act(() => {
    if (!el) return;
    el.value = value;
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });
const wheel = () => {
  const w = q("[data-wheel]");
  if (w)
    w.getBoundingClientRect = () => ({
      left: 0,
      top: 0,
      width: 200,
      height: 200,
      right: 200,
      bottom: 200,
      x: 0,
      y: 0,
      toJSON() {},
    });
  return w;
};

beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
  actions = null;
  vi.mocked(api.getLibrary).mockResolvedValue(lib());
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  vi.mocked(api.previewPalette).mockResolvedValue(undefined);
});
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
  vi.clearAllMocks();
});

it("wheel_maps_hue_and_saturation", async () => {
  await mount();
  const w = wheel();
  await pointer(w, "pointerdown", 200, 100); // east edge: hue 0, full saturation
  expect(pal(1)?.principal).toEqual({ h: 0, s: 1000, v: 1000 });
  await pointer(w, "pointermove", 100, 150); // south, half way: hue 90, saturation 500
  expect(pal(1)?.principal).toEqual({ h: 90, s: 500, v: 1000 });
  await pointer(w, "pointerup", 100, 150);
  await pointer(w, "pointermove", 0, 100); // after release nothing follows the pointer
  expect(pal(1)?.principal.h).toBe(90);
  await store.undo();
  expect(pal(1)?.principal).toEqual(hsv(300)); // the whole drag was one undo step
});

it("brightness_slider_sets_v_only", async () => {
  await mount();
  expect(q("[data-brightness]")?.textContent).toContain("100 %");
  await typeInto(q<HTMLInputElement>("input[aria-label='Brightness']"), "400");
  expect(pal(1)?.principal).toEqual({ h: 300, s: 1000, v: 400 });
  expect(q("[data-brightness]")?.textContent).toContain("40 %");
});

it("hex_input_round_trip_and_rejects_invalid", async () => {
  await mount();
  const hex = () => q<HTMLInputElement>("input[aria-label='Hex colour']");
  await typeInto(hex(), "#00FF00");
  expect(pal(1)?.principal).toEqual({ h: 120, s: 1000, v: 1000 });
  expect(q("[role=alert]")).toBeNull();
  await typeInto(hex(), "#zz");
  expect(q("[role=alert]")?.textContent).toContain("#FF2D55");
  expect(pal(1)?.principal).toEqual({ h: 120, s: 1000, v: 1000 });
  await act(() => void hex()?.dispatchEvent(new Event("focusout", { bubbles: true })));
  expect(hex()?.value).toBe("#00FF00");
});

it("drag_role_onto_role_swaps", async () => {
  await mount();
  const d = dt();
  await fire(q("[data-role='principal']"), "dragstart", { dataTransfer: d });
  await fire(q("[data-role='contraste']"), "drop", { dataTransfer: d });
  expect(pal(1)?.principal).toEqual(hsv(180));
  expect(pal(1)?.contraste).toEqual(hsv(300));
  await store.undo();
  expect(pal(1)?.principal).toEqual(hsv(300)); // one undo step
  expect(pal(1)?.contraste).toEqual(hsv(180));
});

it("drop_colour_from_other_palette_copies", async () => {
  await mount();
  const d = dt();
  await fire(q("[data-palette='2'] [data-swatch='suave']"), "dragstart", { dataTransfer: d });
  await fire(q("[data-role='principal']"), "drop", { dataTransfer: d });
  expect(pal(1)?.principal).toEqual(hsv(45, 700, 800));
  expect(pal(2)?.suave).toEqual(hsv(45, 700, 800)); // the source keeps its colour
});

it("used_by_lists_sets", async () => {
  await mount();
  expect(q("[data-used-by]")?.textContent).toContain("Club night");
  await click(q("[data-used-by] button"));
  expect(goSets).toHaveBeenCalled();
  await click(q("[data-palette='3'] button"));
  expect(q("[data-used-by]")?.textContent).toContain("No set uses this palette");
});

it("show_on_strip_turns_off_on_leave", async () => {
  await mount();
  await click(q("button[aria-label='Show on strip']"));
  await tick(300);
  expect(api.previewPalette).toHaveBeenLastCalledWith(true, pal(1), 7);
  await act(async () => render(null, root)); // leaving the tab
  expect(api.previewPalette).toHaveBeenLastCalledWith(false, expect.anything(), 7);
});

it("show_on_strip_turns_off_when_the_window_loses_focus", async () => {
  await mount();
  await click(q("button[aria-label='Show on strip']"));
  await tick(300);
  await act(() => void window.dispatchEvent(new Event("blur")));
  expect(api.previewPalette).toHaveBeenLastCalledWith(false, expect.anything(), 7);
  expect(q("button[aria-label='Show on strip']")?.getAttribute("aria-pressed")).toBe("false");
});

it("delete_is_blocked_while_a_set_uses_the_palette", async () => {
  await mount();
  let blocked: boolean | undefined;
  await act(() => void (blocked = actions?.deleteGuard?.()));
  expect(blocked).toBe(true); // handled: no confirm dialog
  expect(q("[data-notice]")?.textContent).toContain("Club night");
  expect(store.lib?.palettes).toHaveLength(3);
  await click(q("[data-palette='3'] button"));
  await act(() => void (blocked = actions?.deleteGuard?.()));
  expect(blocked).toBe(false);
  await act(() => void actions?.onDelete?.());
  expect(store.lib?.palettes.map(([i]) => i)).toEqual([1, 2]);
});

it("duplicate_copies_with_a_copy_suffix", async () => {
  await mount();
  await act(() => void actions?.onDuplicate?.());
  const names = store.lib?.palettes.map(([, p]) => p.name);
  expect(names).toContain("Neon copy");
  expect(names).toHaveLength(4);
});

it("browser_lists_palettes_with_three_swatches_and_new_palette", async () => {
  await mount();
  expect(root.querySelectorAll("[data-palette]")).toHaveLength(3);
  expect(root.querySelectorAll("[data-palette='1'] [data-swatch]")).toHaveLength(3);
  const byText = [...root.querySelectorAll("button")].find((b) =>
    b.textContent?.includes("New palette"),
  );
  await click(byText);
  expect(store.lib?.palettes).toHaveLength(4);
  expect(root.textContent).toContain("Main colour · most segments");
  expect(root.textContent).toContain("Accents and hits");
  expect(root.textContent).toContain("Background, calm moments");
});

it("a_duplicate_stays_selected_after_renumbering_and_recolour_edits_it", async () => {
  vi.mocked(api.saveLibrary).mockImplementation(renumbering(lib()));
  await mount();
  await act(() => void actions?.onDuplicate?.());
  await act(async () => void (await new Promise((r) => setTimeout(r, 600))));
  expect(store.lib?.palettes.map(([i]) => i)).toEqual([1, 2, 3, 4]);
  await typeInto(q<HTMLInputElement>("input[aria-label='Hex colour']"), "#00FF00");
  expect(pal(4)?.principal).toEqual({ h: 120, s: 1000, v: 1000 });
  expect(pal(1)?.principal).toEqual({ h: 300, s: 1000, v: 1000 });
});
