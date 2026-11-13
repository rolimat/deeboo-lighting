import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library, Sequence } from "../api";

vi.mock("../api", () => ({
  api: {
    getLibrary: vi.fn(),
    saveLibrary: vi.fn(),
    messageCounts: vi.fn(),
    patternSequence: vi.fn(),
    previewGenerated: vi.fn(),
    getFullStatus: vi.fn(async () => ({ bpm: null, playing: false })),
    onStatus: vi.fn(async () => () => {}),
  },
}));
import { api } from "../api";
import { useLibrary } from "../state/library";
import { SequencesTab, type BarActions } from "./SequencesTab";

const hsv = (h: number) => ({ h, s: 1000, v: 1000 });
const roles = (...r: string[]) => r;
const pulse = (): Sequence => ({
  name: "Pulse",
  moment: "rhythm",
  steps: [
    { len: "one", roles: roles("principal", "off", "off") },
    { len: "two", roles: roles("off", "off", "off") },
    { len: "four", roles: roles("off", "suave", "off") },
    { len: "one", roles: roles("off", "off", "off") },
  ],
});
const lib = (): Library => ({
  schemaVersion: 2,
  shape: { pieces: Array.from({ length: 3 }, (_, i) => ({ x: i * 2, y: 0, angle: 0, len: 1 })) },
  palettes: [
    [1, { name: "Neon", principal: hsv(300), contraste: hsv(180), suave: hsv(260) }],
    [2, { name: "Sunset", principal: hsv(20), contraste: hsv(340), suave: hsv(45) }],
  ],
  sequences: [
    [7, pulse()],
    [
      8,
      {
        name: "Drift",
        moment: "calm",
        steps: [
          { len: "four", roles: roles("suave", "off", "off") },
          { len: "four", roles: roles("off", "off", "off") },
        ],
      },
    ],
  ],
  sets: [
    {
      id: 1,
      name: "Club night",
      rhythm: { sequences: [{ id: 7 }, "generated"], order: "inOrder", interval: { bars: 4 } },
      calm: { sequences: [{ id: 8 }], order: "inOrder", interval: { bars: 4 } },
      buildup: { sequences: ["generated"], order: "inOrder", interval: { bars: 4 } },
      palettes: [2, 1],
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
function Harness() {
  store = useLibrary();
  return (
    <SequencesTab
      store={store}
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
const fire = (el: Element | null, type: string, init: MouseEventInit = {}) =>
  act(() => void el?.dispatchEvent(new MouseEvent(type, { bubbles: true, ...init })));
const q = <T extends Element = HTMLElement>(sel: string) => root.querySelector<T>(sel);
const cell = (seg: number, col: number) => q(`[data-seg="${seg}"][data-col="${col}"]`);
const role = (seg: number, col: number) =>
  cell(seg, col)?.getAttribute("aria-label")?.split(": ")[1];
const byText = (sel: string, text: string) =>
  [...root.querySelectorAll<HTMLElement>(sel)].find((e) => e.textContent?.trim() === text);
const chip = (name: string) => q(`button[aria-label="Paint ${name}"]`);

beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
  actions = null;
  vi.mocked(api.getLibrary).mockResolvedValue(lib());
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  vi.mocked(api.messageCounts).mockImplementation(async (s) => s.steps.map(() => 1));
  vi.mocked(api.patternSequence).mockResolvedValue({
    name: "Halves",
    moment: "rhythm",
    steps: [
      { len: "four", roles: roles("principal", "off", "principal") },
      { len: "four", roles: roles("off", "principal", "off") },
    ],
  });
  vi.mocked(api.previewGenerated).mockResolvedValue(pulse());
});
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
  vi.clearAllMocks();
});

it("browser_groups_by_moment_with_icons", async () => {
  await mount();
  const nav = q("nav[aria-label='Sequences']");
  const heads = [...(nav?.querySelectorAll("h3") ?? [])];
  expect(heads.map((h) => h.textContent?.trim())).toEqual(["Rhythm", "Calm", "Build-up"]);
  for (const h of heads) expect(h.querySelector("svg")).not.toBeNull();
  const rhythm = heads[0]?.nextElementSibling as HTMLElement;
  expect(rhythm.textContent).toContain("Pulse");
  expect(rhythm.textContent).toContain("Halves"); // one entry per pattern
  expect(rhythm.textContent).toContain("Any pattern");
  expect(heads[2]?.nextElementSibling?.textContent).toContain("Fast snake");
  // generated entries carry the teal icon, user sequences do not
  expect(rhythm.querySelector("[data-generated] svg")).not.toBeNull();
  expect(byText("nav button", "Pulse")?.querySelector("[data-generated]")).toBeNull();
  expect(byText("button", "Pulse")?.getAttribute("aria-current")).toBe("true");
});

it("browser_items_show_grip_and_moment_icon", async () => {
  await mount();
  for (const label of ["Pulse", "Drift", "Halves", "Fast snake"]) {
    const b = byText("nav button", label);
    expect(b?.querySelector("[data-grip]"), label).not.toBeNull();
    expect(b?.querySelector("[data-moment-icon] svg"), label).not.toBeNull();
  }
});

it("off_cells_are_filled_rectangles", async () => {
  await mount();
  const off = cell(1, 0) as HTMLElement; // Pulse: S2 at 1.1 is off
  expect(role(1, 0)).toBe("Off");
  expect(off.style.background).toContain("var(--color-off)");
});

it("name_label_left_of_input", async () => {
  await mount();
  const label = q("label[for]");
  const input = q<HTMLInputElement>("input[aria-label='Name'], input#seq-name");
  expect(label?.textContent?.trim().toLowerCase()).toBe("name");
  expect(label?.getAttribute("for")).toBe(input?.id);
  expect(label?.parentElement).toBe(input?.parentElement); // same row
  expect(label?.nextElementSibling).toBe(input);
  expect(input?.value).toBe("Pulse");
});

it("grid_fits_without_horizontal_scroll", async () => {
  await mount();
  const grid = q("[data-grid]");
  expect(grid?.style.minWidth).toBe("");
  // Every cell is a flexible grid track, never a fixed pixel width.
  for (const el of root.querySelectorAll<HTMLElement>("[style]"))
    expect(el.style.width).not.toMatch(/px$/);
  expect(grid?.style.gridTemplateColumns).toContain("minmax(0");
  expect(q<HTMLButtonElement>("button[aria-label='Zoom out']")?.disabled).toBe(true);
  await click(q("button[aria-label='Zoom in']"));
  expect(q("[data-grid]")?.style.minWidth).toBe("200%");
});

it("drag_edge_adds_whole_bar_copying_last", async () => {
  await mount();
  const clip = q("[data-clip]");
  expect(clip?.textContent).toContain("Pulse · 2 bars · loops");
  expect(root.querySelectorAll("[data-seg='0']")).toHaveLength(16);
  const handle = q("[role=slider][aria-label='Clip length']");
  await act(
    () =>
      void handle?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
      ),
  );
  expect(q("[data-clip]")?.textContent).toContain("3 bars");
  expect(root.querySelectorAll("[data-seg='0']")).toHaveLength(24);
  // new bar = copy of the last (bar 2 has a suave on S2 at 2.1)
  expect(role(1, 16 + 0)).toBe(role(1, 8));
  expect(role(1, 8)).toBe("Soft");
  // pointer drag: grid 800px wide for 3 bars; drag to 1 bar worth
  const grid = q("[data-clip-track]");
  (grid as HTMLElement).getBoundingClientRect = () => ({
    left: 0,
    width: 600,
    right: 600,
    top: 0,
    bottom: 0,
    height: 0,
    x: 0,
    y: 0,
    toJSON() {},
  });
  await fire(q("[role=slider]"), "pointerdown", { clientX: 600 });
  await fire(document.body, "pointermove", { clientX: 200 });
  await fire(document.body, "pointerup", { clientX: 200 });
  expect(q("[data-clip]")?.textContent).toContain("1 bar ·");
  expect(root.querySelectorAll("[data-seg='0']")).toHaveLength(8);
  await act(
    () =>
      void handle?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true })),
  );
  expect(q("[data-clip]")?.textContent).toContain("1 bar"); // never below one bar
});

it("paint_by_drag_any_direction_one_undo_step", async () => {
  await mount();
  await click(chip("Contrast"));
  await fire(cell(2, 5), "pointerdown");
  await fire(cell(1, 5), "pointerenter"); // up
  await fire(cell(1, 4), "pointerenter"); // then left
  await fire(document.body, "pointerup");
  expect([role(2, 5), role(1, 5), role(1, 4)]).toEqual(["Contrast", "Contrast", "Contrast"]);
  await fire(cell(0, 0), "pointerenter"); // released: no paint
  expect(role(0, 0)).toBe("Principal");
  await act(() => store.undo());
  expect([role(2, 5), role(1, 5), role(1, 4)]).toEqual(["Off", "Off", "Off"]);
  expect(store.canUndo).toBe(false); // one undo step covered the whole drag
});

it("multi_message_warning_on_column", async () => {
  vi.mocked(api.messageCounts).mockImplementation(async (s) =>
    s.steps.map((_, i) => (i === 2 ? 3 : 1)),
  );
  await mount();
  await tick(300);
  const warn = q("[data-col='6'][role=status]"); // step 3 starts at half-beat 6
  expect(warn?.textContent).toContain("Shown in 3 parts");
  expect(root.querySelectorAll("[role=status]")).toHaveLength(1);
});

it("generated_opens_read_only_with_duplicate_to_edit", async () => {
  await mount();
  await click(byText("nav button", "Halves"));
  await tick();
  expect(api.patternSequence).toHaveBeenCalledWith("halves", expect.any(Number));
  expect(q("input#seq-name")).toBeNull();
  const before = lib().sequences.length;
  await fire(cell(0, 0), "pointerdown"); // read-only: nothing painted
  expect(store.canUndo).toBe(false);
  await click(byText("button", "Duplicate to edit"));
  await tick();
  expect(store.lib?.sequences).toHaveLength(before + 1);
  const added = store.lib?.sequences[before]?.[1];
  expect(added).toMatchObject({ name: "Halves", moment: "rhythm" });
  expect(q<HTMLInputElement>("input#seq-name")?.value).toBe("Halves"); // selected, editable
  expect(byText("button", "Duplicate to edit")).toBeUndefined();
});

it("moment_change_warns_and_removes_from_other_tracks", async () => {
  await mount();
  await click(q("button[aria-label='Moment Calm']"));
  const dlg = q("[role=dialog]");
  expect(dlg?.textContent).toContain("Used by Club night (Rhythm). It will be removed there.");
  await click(byText("[role=dialog] button", "Cancel"));
  expect(store.lib?.sequences[0]?.[1].moment).toBe("rhythm");
  await click(q("button[aria-label='Moment Calm']"));
  await click(byText("[role=dialog] button", "Change moment"));
  expect(store.lib?.sequences[0]?.[1].moment).toBe("calm");
  expect(store.lib?.sets[0]?.rhythm.sequences).toEqual(["generated"]);
  await act(() => store.undo()); // sequence and sets change together
  expect(store.lib?.sequences[0]?.[1].moment).toBe("rhythm");
  expect(store.lib?.sets[0]?.rhythm.sequences).toEqual([{ id: 7 }, "generated"]);
});

it("delete_is_blocked_while_a_set_uses_the_sequence", async () => {
  await mount();
  expect(actions?.deleteGuard?.()).toBe(true); // TopBar skips the confirm dialog
  await tick();
  expect(store.lib?.sequences).toHaveLength(2);
  expect(root.textContent).toContain("Used by Club night");
});

it("duplicate_and_delete_act_on_the_selection", async () => {
  await mount();
  actions?.onDuplicate?.();
  await tick();
  expect(store.lib?.sequences).toHaveLength(3);
  expect(q<HTMLInputElement>("input#seq-name")?.value).toBe("Pulse copy");
  expect(actions?.deleteLabel).toBe("Pulse copy");
  actions?.onDelete?.(); // the copy is in no set
  await tick();
  expect(store.lib?.sequences).toHaveLength(2);
  await click(byText("nav button", "Halves"));
  await tick();
  expect(actions?.onDuplicate).toBeUndefined(); // generated: nothing to duplicate or delete
  expect(actions?.onDelete).toBeUndefined();
});

it("dropping_a_palette_changes_the_preview", async () => {
  await mount();
  const sel = q<HTMLSelectElement>("select#seq-palette");
  expect(sel?.value).toBe("2"); // first palette of the active set
  const item = q("[data-palette='1']");
  const data = new Map<string, string>();
  const dt = {
    setData: (k: string, v: string) => data.set(k, v),
    getData: (k: string) => data.get(k) ?? "",
  };
  await act(() => {
    const e = new Event("dragstart", { bubbles: true });
    Object.assign(e, { dataTransfer: dt });
    item?.dispatchEvent(e);
  });
  await act(() => {
    const e = new Event("drop", { bubbles: true, cancelable: true });
    Object.assign(e, { dataTransfer: dt });
    q("[data-grid]")?.dispatchEvent(e);
  });
  expect(q<HTMLSelectElement>("select#seq-palette")?.value).toBe("1");
  expect(store.canUndo).toBe(false); // UI-only
});

const bw = (sel: string) => q<HTMLButtonElement>(`button[aria-label='${sel}']`);

it("double_and_halve_buttons_undoable_with_limits", async () => {
  await mount();
  await click(bw("Double length"));
  expect(q("[data-clip]")?.textContent).toContain("4 bars");
  expect(role(0, 0)).toBe(role(0, 16)); // content repeated
  await act(() => store.undo());
  expect(q("[data-clip]")?.textContent).toContain("2 bars");
  await click(bw("Halve length"));
  expect(q("[data-clip]")?.textContent).toContain("1 bar ·");
  expect(bw("Halve length")?.disabled).toBe(true);
  for (const want of ["2 bars", "4 bars", "8 bars", "16 bars"]) {
    await click(bw("Double length"));
    expect(q("[data-clip]")?.textContent).toContain(want);
  }
  expect(bw("Double length")?.disabled).toBe(true);
});

it("drag_reaches_beyond_the_end_and_back_to_original", async () => {
  await mount();
  const track = q("[data-clip-track]") as HTMLElement;
  // 2 bars fill 400px of a 800px grid with room to the right
  track.getBoundingClientRect = () => ({
    left: 0,
    width: 400,
    right: 400,
    top: 0,
    bottom: 0,
    height: 0,
    x: 0,
    y: 0,
    toJSON() {},
  });
  await fire(q("[role=slider]"), "pointerdown", { clientX: 400 });
  await fire(document.body, "pointermove", { clientX: 600 });
  expect(q("[data-clip]")?.textContent).toContain("3 bars");
  await fire(document.body, "pointermove", { clientX: 400 });
  expect(q("[data-clip]")?.textContent).toContain("2 bars"); // back to the original length
  await fire(document.body, "pointerup", { clientX: 400 });
});

it("grid_leaves_room_to_the_right_of_the_clip", async () => {
  await mount();
  const cols = (q("[data-grid]") as HTMLElement).style.gridTemplateColumns;
  expect(cols).toContain("repeat(24"); // 2 bars + 1 spare
  expect(root.querySelectorAll("[data-seg='0']")).toHaveLength(16);
  // every segment row has exactly 3 bars of columns; only the first 2 are paintable
  const g = q("[data-grid]") as HTMLElement;
  const kids = Array.from(g.children).filter(
    (el) => el.matches("button[data-seg]") || el.matches("[data-room]"),
  );
  const segs = root.querySelectorAll("[data-col='0'][data-seg]").length;
  expect(kids).toHaveLength(segs * 24);
  kids.forEach((el, i) => {
    const inRoom = i % 24 >= 16;
    expect(el.matches("[data-room]")).toBe(inRoom);
    if (!inRoom) expect(Number(el.getAttribute("data-seg"))).toBe(Math.floor(i / 24));
  });
  expect(g.querySelectorAll("[data-room] button, button[data-room]")).toHaveLength(0);
});

it("unchanged_length_is_not_an_edit", async () => {
  await mount();
  const track = q("[data-clip-track]") as HTMLElement;
  track.getBoundingClientRect = () => ({
    left: 0,
    width: 400,
    right: 400,
    top: 0,
    bottom: 0,
    height: 0,
    x: 0,
    y: 0,
    toJSON() {},
  });
  await fire(q("[role=slider]"), "pointerdown", { clientX: 400 });
  await fire(document.body, "pointermove", { clientX: 400 }); // same 2 bars
  await fire(document.body, "pointerup", { clientX: 400 });
  expect(store.canUndo).toBe(false);
  // ArrowRight at 16 bars adds no undo entry
  for (let i = 0; i < 4; i++) await click(bw("Double length"));
  await act(() => store.undo());
  await act(() => store.undo());
  await act(() => store.undo());
  await act(() => store.undo());
  expect(store.canUndo).toBe(false);
  await act(() => store.redo());
  await act(() => store.redo());
  await act(() => store.redo());
  await act(() => store.redo());
  expect(q("[data-clip]")?.textContent).toContain("16 bars");
  await act(() => store.undo()); // 8 bars, so redo is available
  await act(() => store.redo()); // back at 16
  const h = q("[role=slider]");
  await act(
    () => void h?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })),
  );
  await act(() => store.undo());
  expect(q("[data-clip]")?.textContent).toContain("8 bars"); // one undo undid the last real edit, not a no-op
});
