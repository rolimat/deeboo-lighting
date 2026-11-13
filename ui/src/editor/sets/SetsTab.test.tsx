import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library } from "../api";

vi.mock("../api", () => ({
  api: {
    getLibrary: vi.fn(),
    saveLibrary: vi.fn(),
    setLiveSet: vi.fn(),
    previewSequence: vi.fn(),
    getFullStatus: vi.fn(async () => ({ bpm: null, playing: false })),
    onStatus: vi.fn(async () => () => {}),
  },
}));
import { api } from "../api";
import { useLibrary } from "../state/library";
import { SetsTab } from "./SetsTab";
import type { BarActions } from "../sequences/SequencesTab";

const hsv = (h: number) => ({ h, s: 1000, v: 1000 });
const seq = (name: string, moment: string) => ({
  name,
  moment,
  steps: [{ len: "four", roles: ["principal", "off"] }],
});
const ms = (sequences: Library["sets"][number]["rhythm"]["sequences"], bars = 4) => ({
  sequences,
  order: "inOrder" as const,
  interval: { bars },
});
const lib = (): Library => ({
  schemaVersion: 2,
  shape: { pieces: [] },
  palettes: [
    [1, { name: "Neon", principal: hsv(300), contraste: hsv(180), suave: hsv(260) }],
    [2, { name: "Sunset", principal: hsv(20), contraste: hsv(340), suave: hsv(45) }],
    [3, { name: "Ice", principal: hsv(200), contraste: hsv(0), suave: hsv(220) }],
  ],
  sequences: [
    [10, seq("Pulse", "rhythm")],
    [11, seq("Drift", "calm")],
    [12, seq("Rise", "buildUp")],
    [13, seq("Chase", "rhythm")],
  ],
  sets: [
    {
      id: 1,
      name: "Main",
      rhythm: ms([{ id: 10 }, { id: 13 }]),
      calm: ms([{ id: 11 }]),
      buildup: ms([{ id: 12 }]),
      palettes: [1, 2],
      paletteOrder: "inOrder",
      paletteChange: "atDrop",
    },
    {
      id: 2,
      name: "Club",
      rhythm: ms(["generated"]),
      calm: ms(["generated"]),
      buildup: ms(["generated"]),
      palettes: [1],
      paletteOrder: "inOrder",
      paletteChange: "atDrop",
    },
  ],
  activeSet: 1,
  activePalette: null,
});

let root: HTMLElement;
let store: ReturnType<typeof useLibrary>;
let actions: BarActions | null = null;
const settle = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function Harness({ onOpen }: { onOpen?: () => void }) {
  store = useLibrary();
  return (
    <SetsTab
      store={store}
      onOpenSequence={onOpen}
      onActions={(a) => {
        actions = a;
      }}
    />
  );
}
async function mount(onOpen?: () => void) {
  await act(async () => {
    render(<Harness onOpen={onOpen} />, root);
    await settle();
  });
  await act(async () => {
    await settle(10);
  });
}
const q = <T extends Element = HTMLElement>(sel: string) => root.querySelector<T>(sel);
const qa = (sel: string) => [...root.querySelectorAll<HTMLElement>(sel)];
const click = (el: Element | null | undefined) =>
  act(async () => {
    (el as HTMLElement).click();
    await settle();
  });
const fire = (el: Element | null | undefined, type: string, extra: object = {}) =>
  act(
    () =>
      void el?.dispatchEvent(
        Object.assign(new Event(type, { bubbles: true, cancelable: true }), extra, {
          dataTransfer: { setData() {}, getData: () => "", effectAllowed: "" },
        }),
      ),
  );
const dragDrop = async (from: Element | null, to: Element | null) => {
  await fire(from, "dragstart");
  await fire(to, "dragover");
  await fire(to, "drop");
  await fire(from, "dragend");
};
const status = (clip: object | null) => ({
  bpm: 120,
  playing: true,
  moment: "rhythm",
  clip,
  selection: { palette: { name: "Neon" } },
});
const refs = (m: "rhythm" | "calm" | "buildup", set = 0) => store.lib?.sets[set]?.[m].sequences;

beforeEach(async () => {
  root = document.createElement("div");
  document.body.append(root);
  actions = null;
  vi.mocked(api.getLibrary).mockResolvedValue(lib());
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  vi.mocked(api.setLiveSet).mockResolvedValue({} as never);
  vi.mocked(api.previewSequence).mockResolvedValue();
  await mount();
});
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
  vi.clearAllMocks();
});

it("calm_sequence_cannot_drop_on_rhythm", async () => {
  await dragDrop(q("[data-drag-seq='11']"), q("[data-track='rhythm']"));
  expect(refs("rhythm")).toEqual([{ id: 10 }, { id: 13 }]);
  await fire(q("[data-drag-seq='11']"), "dragstart");
  expect(q("[data-track='rhythm'] [data-zone]")?.textContent).toBe("Rhythm only");
  expect(q("[data-track='buildup'] [data-zone]")?.textContent).toBe("Build-up only");
  expect(q("[data-track='calm'] [data-zone]")?.textContent).toBe('Drop "Drift" here');
});

it("drop_appends_clip", async () => {
  expect(q("[data-track='rhythm'] [data-zone]")?.textContent).toBe("Drop sequences here");
  await dragDrop(q("[data-drag-seq='13']"), q("[data-track='rhythm'] [data-zone]"));
  expect(refs("rhythm")).toEqual([{ id: 10 }, { id: 13 }, { id: 13 }]);
  await dragDrop(q("[data-drag-pattern='snake']"), q("[data-track='rhythm'] [data-zone]"));
  expect(refs("rhythm")?.at(-1)).toEqual({ pattern: "snake" });
});

it("drag_reorders_clips", async () => {
  await dragDrop(q("[data-clip='rhythm-1']"), q("[data-clip='rhythm-0']"));
  expect(refs("rhythm")).toEqual([{ id: 13 }, { id: 10 }]);
});

it("remove_clip_x", async () => {
  await click(q("button[aria-label='Remove Chase']"));
  expect(refs("rhythm")).toEqual([{ id: 10 }]);
  await click(q("button[aria-label='Remove Pulse']"));
  expect(refs("rhythm")).toEqual([{ id: 10 }]);
  expect(root.textContent).toContain("A track needs at least one sequence.");
});

it("order_and_change_every_on_one_row", async () => {
  const row = q("[data-track='rhythm'] [data-options]");
  expect(row?.querySelector("button[aria-label='Random']")).toBeTruthy();
  expect(row?.querySelector("select[aria-label='Rhythm change every']")).toBeTruthy();
  await click(row?.querySelector("button[aria-label='Random']"));
  expect(store.lib?.sets[0]?.rhythm.order).toBe("random");
  const sel = row?.querySelector("select") as HTMLSelectElement;
  await act(async () => {
    sel.value = "8";
    sel.dispatchEvent(new Event("change", { bubbles: true }));
  });
  expect(store.lib?.sets[0]?.rhythm.interval).toEqual({ bars: 8 });
});

it("calm_offers_once_per_break_only_calm", async () => {
  const opts = (m: string) =>
    qa(`[data-track='${m}'] select option`).map((o) => (o as HTMLOptionElement).value);
  expect(opts("calm")).toContain("oncePerBreak");
  expect(opts("rhythm")).not.toContain("oncePerBreak");
  expect(opts("buildup")).not.toContain("oncePerBreak");
  const sel = q<HTMLSelectElement>("select[aria-label='Calm change every']") as HTMLSelectElement;
  await act(async () => {
    sel.value = "oncePerBreak";
    sel.dispatchEvent(new Event("change", { bubbles: true }));
  });
  expect(store.lib?.sets[0]?.calm.interval).toBe("oncePerBreak");
});

it("palette_row_full_width_wraps_with_drop_zone_last", async () => {
  const row = q("[data-palette-row]");
  expect(row?.closest("[data-track]")).toBeNull();
  const chips = row?.querySelector("[data-chips]");
  expect(chips?.className).toContain("flex-wrap");
  expect(chips?.lastElementChild?.hasAttribute("data-palette-drop")).toBe(true);
  await dragDrop(q("[data-drag-palette='3']"), q("[data-palette-drop]"));
  expect(store.lib?.sets[0]?.palettes).toEqual([1, 2, 3]);
  await click(q("button[aria-label='Remove Ice']"));
  expect(store.lib?.sets[0]?.palettes).toEqual([1, 2]);
});

it("play_live_calls_set_live_set_and_not_save", async () => {
  vi.mocked(api.getLibrary).mockResolvedValue({ ...lib(), activeSet: 2 });
  await click(q("button[aria-label='Play Club live']"));
  expect(api.setLiveSet).toHaveBeenCalledWith(2);
  await act(async () => void (await settle(450)));
  expect(api.saveLibrary).not.toHaveBeenCalled();
  expect(q("button[aria-label='Playing Club live']")).toBeTruthy();
});

it("click_name_opens_without_changing_live", async () => {
  expect(q<HTMLInputElement>("input[aria-label='Set name']")?.value).toBe("Main");
  expect(q("[data-live-state]")?.textContent).toContain("Playing live");
  await click(qa("nav button").find((b) => b.textContent === "Club"));
  expect(q<HTMLInputElement>("input[aria-label='Set name']")?.value).toBe("Club");
  expect(q("[data-live-state]")?.textContent).toContain("Not playing");
  expect(api.setLiveSet).not.toHaveBeenCalled();
});

it("hold_preview_starts_and_stops", async () => {
  const b = q("button[aria-label='Preview Pulse on strip']");
  await fire(b, "pointerdown");
  expect(api.previewSequence).toHaveBeenLastCalledWith(
    true,
    expect.objectContaining({ name: "Neon" }),
    { id: 10 },
    "rhythm",
  );
  await fire(b, "pointerup");
  expect(api.previewSequence).toHaveBeenLastCalledWith(
    false,
    expect.anything(),
    { id: 10 },
    "rhythm",
  );
  await fire(b, "pointerdown");
  await act(async () => render(null, root)); // unmount while held
  expect(api.previewSequence).toHaveBeenLastCalledWith(
    false,
    expect.anything(),
    { id: 10 },
    "rhythm",
  );
});

it("double_click_clip_opens_it_in_sequences", async () => {
  const open = vi.fn();
  await act(async () => render(null, root));
  await mount(open);
  await fire(q("[data-clip='rhythm-0']"), "dblclick");
  expect(open).toHaveBeenCalledWith({ k: "seq", id: 10 });
});

it("new_set_has_the_documented_defaults_and_opens", async () => {
  await click(qa("nav button").find((b) => b.textContent?.includes("New set")));
  const s = store.lib?.sets.at(-1);
  expect(s).toMatchObject({
    name: "New set",
    rhythm: { sequences: ["generated"], order: "inOrder", interval: { bars: 8 } },
    calm: { sequences: ["generated"], interval: "oncePerBreak" },
    buildup: { sequences: ["generated"], interval: { bars: 2 } },
    palettes: [1, 2, 3],
    paletteOrder: "inOrder",
    paletteChange: "atDrop",
  });
  expect(q<HTMLInputElement>("input[aria-label='Set name']")?.value).toBe("New set");
});

it("duplicate_and_delete_act_on_the_open_set_and_last_set_is_protected", async () => {
  await act(async () => actions?.onDuplicate?.());
  expect(store.lib?.sets.map((s) => s.name)).toEqual(["Main", "Club", "Main copy"]);
  await act(async () => actions?.onDelete?.());
  expect(store.lib?.sets.map((s) => s.name)).toEqual(["Main", "Club"]);
  await click(qa("nav button").find((b) => b.textContent === "Club"));
  await act(async () => actions?.onDelete?.());
  expect(store.lib?.sets.map((s) => s.name)).toEqual(["Main"]);
  let guarded: boolean | undefined;
  await act(async () => void (guarded = actions?.deleteGuard?.()));
  expect(guarded).toBe(true);
  expect(root.textContent).toContain("You need at least one set.");
});

it("browser_and_main_pane_are_separate_scroll_containers", () => {
  const nav = q("nav[aria-label='Sets']");
  expect(nav?.className).toContain("overflow-y-auto");
  const main = q("[data-main]");
  expect(main?.className).toContain("min-h-0");
  const tracks = q("[data-tracks]");
  expect(tracks?.className).toContain("overflow-y-auto");
  expect(main?.contains(tracks as Node)).toBe(true);
  // the palette row is pinned: a sibling after the scrolling tracks, never inside them
  expect(tracks?.contains(q("[data-palette-row]"))).toBe(false);
  expect(q("[data-palette-row]")?.className).toContain("shrink-0");
  expect(nav?.parentElement?.className).toContain("h-[calc(100%+3rem)]");
});

it("order_and_change_every_never_wrap", () => {
  for (const f of ["rhythm", "calm", "buildup"]) {
    const row = q(`[data-track='${f}'] [data-options]`);
    expect(row?.className).toContain("flex-nowrap");
    expect(row?.className).not.toContain("flex-wrap");
  }
});

it("clip_mini_preview_shows_the_first_steps_segments", () => {
  const pills = qa("[data-clip='rhythm-0'] [data-mini] i");
  expect(pills).toHaveLength(2); // one per segment of the first step
  expect(pills[0]?.getAttribute("style")).not.toContain("--color-off");
  expect(pills[1]?.getAttribute("style")).toContain("--color-off");
});

it("playing_clip_glows_with_progress_only_on_the_live_set", async () => {
  vi.mocked(api.getFullStatus).mockResolvedValue(
    status({ moment: "rhythm", index: 1, elapsed: 4, total: 16 }) as never,
  );
  await act(async () => render(null, root));
  await mount();
  const clip = q("[data-clip='rhythm-1']");
  expect(clip?.hasAttribute("data-playing")).toBe(true);
  expect(clip?.className).toContain("border-accent");
  expect(clip?.querySelector<HTMLElement>("[data-progress]")?.style.width).toBe("25%");
  expect(q("[data-clip='rhythm-0']")?.hasAttribute("data-playing")).toBe(false);
  await click(qa("nav button").find((b) => b.textContent === "Club"));
  expect(root.querySelector("[data-playing]")).toBeNull();
});

it("the_live_set_follows_the_status_when_the_popover_changes_it", async () => {
  let push: (s: never) => void = () => {};
  vi.mocked(api.onStatus).mockImplementation(async (cb) => {
    push = cb as never;
    return () => {};
  });
  await act(async () => render(null, root));
  await mount();
  expect(q("button[aria-label='Playing Main live']")).toBeTruthy();
  await click(qa("nav button").find((b) => b.textContent === "Main")); // pin Main as the open set
  const st = { ...status({ moment: "rhythm", index: 1, elapsed: 4, total: 16 }) };
  st.selection = { palette: { name: "Neon" }, setId: 2 } as never;
  await act(async () => void push(st as never));
  expect(q("button[aria-label='Playing Club live']")).toBeTruthy();
  expect(q("button[aria-label='Play Main live']")).toBeTruthy();
  // Main is open but no longer live: no playing clip is drawn on it.
  expect(root.querySelector("[data-playing]")).toBeNull();
  expect(q("[data-live-state]")?.textContent).toContain("Not playing");
});

it("internal_drags_carry_no_plain_text_for_inputs_to_swallow", async () => {
  const data = new Map<string, string>();
  const dt = {
    setData: (k: string, v: string) => data.set(k, v),
    getData: () => "",
    effectAllowed: "",
  };
  const item = q("[data-seq]") ?? q("[draggable='true']");
  await act(() => {
    const e = Object.assign(new Event("dragstart", { bubbles: true }), { dataTransfer: dt });
    item?.dispatchEvent(e);
  });
  expect(data.size).toBeGreaterThan(0);
  expect([...data.keys()]).not.toContain("text/plain");
});
