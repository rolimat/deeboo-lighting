import { afterEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Settings, Status } from "../../api";

vi.mock("../api", () => ({
  api: {
    getSettings: vi.fn(),
    saveSettings: vi.fn(),
    getFullStatus: vi.fn(),
    onStatus: vi.fn(async () => () => {}),
  },
}));
import { api } from "../api";
import { TopBar } from "./TopBar";
import { HelpProvider } from "./Help";

const settings = { helpTooltips: true } as Settings;
const save = { state: "saved" as const };

async function mount(ui: preact.ComponentChild) {
  vi.mocked(api.getSettings).mockResolvedValue(settings);
  vi.mocked(api.saveSettings).mockImplementation(async (s) => s);
  vi.mocked(api.getFullStatus).mockResolvedValue({ bpm: null } as unknown as Status);
  const root = document.createElement("div");
  document.body.append(root);
  await act(async () => {
    render(<HelpProvider>{ui}</HelpProvider>, root);
    await new Promise((r) => setTimeout(r));
  });
  return root;
}
afterEach(() => {
  document.body.innerHTML = "";
  vi.resetAllMocks();
});

const bar = (extra: Partial<Parameters<typeof TopBar>[0]> = {}) => (
  <TopBar
    tab="sets"
    onTab={() => {}}
    canUndo
    canRedo={false}
    onUndo={() => {}}
    onRedo={() => {}}
    save={save}
    {...extra}
  />
);

it("tabs_in_order", async () => {
  const root = await mount(bar());
  const tabs = [...root.querySelectorAll("[role=tab]")];
  expect(tabs.map((t) => t.textContent)).toEqual([
    "Sets",
    "Sequences",
    "Palettes",
    "Strip",
    "Detection",
    "Setup",
  ]);
  expect(tabs[0]?.getAttribute("aria-selected")).toBe("true");
  expect(tabs[0]?.querySelector("svg")).not.toBeNull();
});

it("help_toggle_persists", async () => {
  const root = await mount(bar());
  const help = root.querySelector<HTMLElement>("[aria-label='Help']");
  expect(help?.getAttribute("aria-pressed")).toBe("true");
  await act(async () => help?.click());
  expect(api.saveSettings).toHaveBeenCalledWith({ ...settings, helpTooltips: false });
  expect(help?.getAttribute("aria-pressed")).toBe("false");
});

it("delete_asks_for_confirmation", async () => {
  const onDelete = vi.fn();
  const root = await mount(bar({ onDelete, deleteLabel: "Strobe" }));
  await act(async () => root.querySelector<HTMLElement>("[aria-label='Delete']")?.click());
  expect(onDelete).not.toHaveBeenCalled();
  expect(root.querySelector("[role=dialog]")?.textContent).toContain(
    'Delete "Strobe"? You can undo this with ⌘Z.',
  );
  await act(async () =>
    [...root.querySelectorAll("button")].find((b) => b.textContent === "Cancel")?.click(),
  );
  expect(onDelete).not.toHaveBeenCalled();
  await act(async () => root.querySelector<HTMLElement>("[aria-label='Delete']")?.click());
  await act(async () =>
    [...root.querySelectorAll("button")].find((b) => b.textContent === "Delete")?.click(),
  );
  expect(onDelete).toHaveBeenCalledTimes(1);
  expect(root.querySelector("[role=dialog]")).toBeNull();
});

it("duplicate_and_delete_hidden_without_handlers", async () => {
  const root = await mount(bar());
  expect(root.querySelector("[aria-label='Delete']")).toBeNull();
  expect(root.querySelector("[aria-label='Duplicate']")).toBeNull();
});

it("save_error_is_shown_in_red", async () => {
  const root = await mount(bar({ save: { state: "error", reason: "bad palette" } }));
  const el = root.querySelector("[role=status]");
  expect(el?.textContent).toBe("Not saved — bad palette");
  expect(el?.className).toContain("text-red");
});

it("live_readout_formats_status", async () => {
  vi.mocked(api.onStatus).mockImplementation(async (cb) => {
    cb({
      bpm: 122,
      playing: true,
      beatInBar: 2,
      barInPhrase: 3,
      phraseBars: 8,
      moment: "rhythm",
    } as unknown as Status);
    return () => {};
  });
  const root = await mount(bar());
  expect(root.querySelector("[aria-label='Live']")?.textContent).toBe(
    "● 122.0 BPM · beat 2 · phrase: bar 3 of 8 · Rhythm",
  );
});

it("live_readout_never_truncates_the_moment", async () => {
  vi.mocked(api.onStatus).mockImplementation(async (cb) => {
    cb({
      bpm: 122,
      playing: true,
      beatInBar: 2,
      barInPhrase: 3,
      phraseBars: 8,
      moment: "calm",
    } as unknown as Status);
    return () => {};
  });
  const root = await mount(bar());
  const live = root.querySelector("[aria-label='Live']");
  const moment = live?.querySelector("[data-moment]");
  expect(moment?.textContent).toBe("Calm");
  expect(moment?.classList.contains("shrink-0")).toBe(true);
  const mid = live?.querySelector("[data-live-text]");
  expect(mid?.classList.contains("truncate")).toBe(true);
  expect(mid?.contains(moment ?? null)).toBe(false);
  expect(mid?.querySelector("[data-beat]")?.className).toContain("max-[1100px]:hidden");
});

it("top_bar_is_a_single_row_of_bordered_icon_buttons", async () => {
  const root = await mount(bar({ onDelete: () => {}, onDuplicate: () => {} }));
  const has = (el: Element | null | undefined, ...cls: string[]) =>
    cls.forEach((c) => expect(el?.classList.contains(c), c).toBe(true));
  for (const name of ["Undo", "Redo", "Duplicate", "Delete", "Help"])
    has(root.querySelector(`[aria-label='${name}']`), "h-7", "w-7", "border", "rounded-lg");
  has(root.querySelector("[aria-label='Delete']"), "text-red", "border-red-dark");
  has(root.querySelector("[aria-label='Help']"), "text-accent", "border-accent");
  has(root.querySelector("[aria-label='Undo']"), "text-dim");
  has(root.querySelector("[role=separator]"), "w-px");
  expect(
    root.querySelector("[role=separator]")?.nextElementSibling?.getAttribute("aria-label"),
  ).toBe("Duplicate");
  has(root.querySelector("[role=status]"), "whitespace-nowrap");
  has(root.querySelector("[aria-label='Live']"), "min-w-0", "bg-card", "rounded-full");
  has(root.querySelector("[role=tab]"), "whitespace-nowrap");
});

it("live_readout_has_a_teal_dot_and_a_bold_moment", async () => {
  vi.mocked(api.onStatus).mockImplementation(async (cb) => {
    cb({
      bpm: 90,
      playing: true,
      beatInBar: 1,
      barInPhrase: 1,
      phraseBars: 4,
      moment: "calm",
    } as unknown as Status);
    return () => {};
  });
  const root = await mount(bar());
  const live = root.querySelector("[aria-label='Live']");
  expect(live?.classList.contains("text-accent")).toBe(false);
  expect(live?.querySelector(".text-accent")?.textContent).toBe("●");
  const moment = live?.querySelector(".font-bold");
  expect(moment?.textContent).toBe("Calm");
  expect(moment?.querySelector("svg")).not.toBeNull();
  expect(live?.textContent).toBe("● 90.0 BPM · beat 1 · phrase: bar 1 of 4 · Calm");
});

it("no_emoji_in_rendered_shell", async () => {
  const root = await mount(bar({ onDelete: () => {}, onDuplicate: () => {} }));
  // ⌘ is the key symbol, ● ○ ✓ — are text glyphs the spec asks for; pictographs are not allowed.
  expect(root.textContent).not.toMatch(/\p{Extended_Pictographic}/u);
});
