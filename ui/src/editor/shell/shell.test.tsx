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

it("no_emoji_in_rendered_shell", async () => {
  const root = await mount(bar({ onDelete: () => {}, onDuplicate: () => {} }));
  // ⌘ is the key symbol, ● ○ ✓ — are text glyphs the spec asks for; pictographs are not allowed.
  expect(root.textContent).not.toMatch(/\p{Extended_Pictographic}/u);
});
