import { afterEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";

vi.mock("./api", () => ({
  api: {
    getStatus: vi.fn(),
    getFullStatus: vi.fn(async () => ({ bpm: null })),
    onStatus: vi.fn(async () => () => {}),
    getSettings: vi.fn(async () => ({ helpTooltips: true })),
    getLibrary: vi.fn(async () => ({
      shape: { pieces: [] },
      palettes: [],
      sequences: [],
      sets: [],
    })),
    onSection: vi.fn(async () => () => {}),
    onWizard: vi.fn(async () => () => {}),
    wizardCancel: vi.fn(),
    wizardState: vi.fn(async () => ({ kind: "idle" })),
    saveLibrary: vi.fn(),
    setWiredSegments: vi.fn(),
    setLiveSet: vi.fn(),
    previewSequence: vi.fn(),
    messageCounts: vi.fn(async () => [1]),
    previewGenerated: vi.fn(),
    patternSequence: vi.fn(),
  },
}));
import { api, type WizardState } from "./api";
import { App } from "./App";
import { renumbering } from "./state/renumber-mock";
import type { Library } from "./api";

async function mount(root: HTMLElement) {
  await act(async () => {
    render(<App />, root);
    await new Promise((r) => setTimeout(r));
  });
}

afterEach(() => {
  vi.restoreAllMocks();
  history.replaceState(null, "", "/");
});

it("renders_strip_tab_by_default", async () => {
  vi.mocked(api.getStatus).mockResolvedValue({
    kind: "connected",
    name: "dj lights",
    segments: 10,
    reported: 10,
  });
  const root = document.createElement("div");
  await mount(root);
  const tab = root.querySelector("[role=tab][aria-selected=true]");
  expect(tab?.textContent).toBe("Strip");
  expect(root.querySelector("section")?.textContent).toContain("dj lights");
  expect(root.querySelector("section")?.textContent).toContain("Connected");
});

it("shows_not_connected_when_no_strip", async () => {
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  await mount(root);
  expect(root.querySelector("section")?.textContent).toContain("not connected");
});

it("strip_tab_stays_on_the_strip_after_a_wired_count_change", async () => {
  const lib = {
    shape: { pieces: [] },
    palettes: [],
    sequences: [],
    sets: [],
  } as unknown as Library;
  vi.mocked(api.getStatus).mockResolvedValue({
    kind: "connected",
    name: "dj lights",
    segments: 4,
    reported: 6,
  });
  vi.mocked(api.setWiredSegments).mockResolvedValue(lib);
  const root = document.createElement("div");
  await mount(root);
  // The strip reconnects after the change: any status read now says "connecting".
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "connecting" });
  const more = root.querySelector<HTMLButtonElement>("[aria-label='More segments']");
  await act(async () => {
    more?.click();
    await new Promise((r) => setTimeout(r, 50));
  });
  expect(api.setWiredSegments).toHaveBeenCalledWith(5);
  const text = root.querySelector("section")?.textContent ?? "";
  expect(text).toContain("dj lights");
  expect(text).not.toContain("Smart Life");
});

it("switches_to_sequences", async () => {
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  await mount(root);
  const tab = [...root.querySelectorAll("[role=tab]")].find((b) => b.textContent === "Sequences");
  await act(async () => (tab as HTMLElement).click());
  expect(root.querySelector("section")?.getAttribute("aria-label")).toBe("Sequences");
  expect(tab?.getAttribute("aria-selected")).toBe("true");
});

it("opens_on_the_section_in_the_url", async () => {
  history.replaceState(null, "", "/?section=palettes");
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  await mount(root);
  expect(root.querySelector("section")?.getAttribute("aria-label")).toBe("Palettes");
});

it("switches_tab_when_the_open_editor_is_told_a_section", async () => {
  let push: (s: string) => void = () => {};
  vi.mocked(api.onSection).mockImplementation(async (cb) => {
    push = cb;
    return () => {};
  });
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  await mount(root);
  await act(async () => push("setup"));
  expect(root.querySelector("section")?.getAttribute("aria-label")).toBe("Setup");
  await act(async () => push("nonsense"));
  expect(root.querySelector("section")?.getAttribute("aria-label")).toBe("Setup");
});

it("wizard_shows_its_own_state_not_runtime_not_connected", async () => {
  let push: (s: WizardState) => void = () => {};
  vi.mocked(api.onWizard).mockImplementation(async (cb) => {
    push = cb;
    return () => {};
  });
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  await mount(root);
  expect(root.querySelector("section")?.textContent).toContain("not connected");
  const device = { id: "a", name: "dj lights", product: "P", online: true, supported: true };
  await act(async () => push({ kind: "listening", device, sinceUs: 1 }));
  const text = root.querySelector("section")?.textContent ?? "";
  expect(text).toContain("Connected to dj lights — waiting for paint data");
  expect(text).not.toContain("not connected");
  await act(async () => push({ kind: "identify", device, segments: 10 }));
  expect(root.querySelector("section")?.textContent).toContain("Connected to dj lights");
  expect(root.querySelector("section")?.textContent).not.toContain("not connected");
});

it("setup_tab_undo_hint_does_not_mention_reset", async () => {
  history.replaceState(null, "", "/?section=setup");
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  await mount(root);
  const undo = root.querySelector("button[aria-label=Undo]");
  expect(undo?.getAttribute("data-help")).toBe("Settings apply at once.");
});

it("a_sequence_opened_from_sets_follows_its_renumbered_id", async () => {
  const color = { h: 0, s: 1000, v: 1000 };
  const ms = (sequences: Library["sets"][number]["rhythm"]["sequences"]) => ({
    sequences,
    order: "inOrder" as const,
    interval: { bars: 4 },
  });
  const seq = { name: "Fresh", moment: "rhythm", steps: [{ len: "four", roles: ["principal"] }] };
  const set = {
    id: 1,
    name: "Main",
    rhythm: ms([{ id: 1_000_001 }]),
    calm: ms(["generated"]),
    buildup: ms(["generated"]),
    palettes: [1],
    paletteOrder: "inOrder",
    paletteChange: "atDrop",
  };
  const lib = {
    schemaVersion: 3,
    shape: { pieces: [{ x: 0, y: 0, angle: 0, len: 1 }] },
    palettes: [[1, { name: "P", principal: color, contraste: color, suave: color }]],
    sequences: [[1_000_001, seq]],
    sets: [set],
    activeSet: 1,
    activePalette: null,
  } as unknown as Library;
  vi.mocked(api.getLibrary).mockResolvedValue(lib);
  vi.mocked(api.saveLibrary).mockImplementation(renumbering({ ...lib, sequences: [] }));
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  history.replaceState(null, "", "/?section=sets");
  const root = document.createElement("div");
  document.body.append(root);
  await mount(root);
  const click = (el?: Element) => act(async () => void (el as HTMLElement).click());
  await act(
    async () =>
      void root
        .querySelector("[data-clip='rhythm-0']")
        ?.dispatchEvent(new Event("dblclick", { bubbles: true })),
  );
  const goto = (name: string) =>
    click([...root.querySelectorAll("[role=tab]")].find((b) => b.textContent === name));
  await goto("Sets");
  const newSet = [...root.querySelectorAll("nav button")].find((b) =>
    b.textContent?.includes("New set"),
  );
  await click(newSet); // the save renumbers 1_000_001 to 1
  await act(async () => void (await new Promise((r) => setTimeout(r, 600))));
  await goto("Sequences");
  expect(root.querySelector<HTMLInputElement>("input#seq-name")?.value).toBe("Fresh");
});

it("a_library_edit_after_a_re_pair_keeps_the_new_segment_count", async () => {
  let push: (s: WizardState) => void = () => {};
  vi.mocked(api.onWizard).mockImplementation(async (cb) => {
    push = cb;
    return () => {};
  });
  const piece = { x: 0, y: 0, angle: 0, len: 1 };
  const ms = {
    sequences: ["generated"],
    order: "inOrder" as const,
    interval: { bars: 4 },
  };
  const set = {
    id: 1,
    name: "Main",
    rhythm: ms,
    calm: ms,
    buildup: ms,
    palettes: [],
    paletteOrder: "inOrder",
    paletteChange: "atDrop",
  };
  const lib = (n: number) =>
    ({
      schemaVersion: 3,
      shape: { pieces: Array.from({ length: n }, () => piece) },
      palettes: [],
      sequences: [],
      sets: [set],
      activeSet: 1,
      activePalette: null,
    }) as unknown as Library;
  vi.mocked(api.getLibrary).mockResolvedValue(lib(1));
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  document.body.append(root);
  await mount(root);
  vi.mocked(api.getLibrary).mockResolvedValue(lib(3)); // Rust rebuilt it for the new strip
  const device = { id: "a", name: "dj", product: "P", online: true, supported: true };
  await act(async () => push({ kind: "saved", device, segments: 3 }));
  const click = (el?: Element) => act(async () => void (el as HTMLElement).click());
  await click([...root.querySelectorAll("[role=tab]")].find((b) => b.textContent === "Sets"));
  await click(
    [...root.querySelectorAll("nav button")].find((b) => b.textContent?.includes("New set")),
  );
  await act(async () => void (await new Promise((r) => setTimeout(r, 600))));
  const sent = vi.mocked(api.saveLibrary).mock.calls.at(-1)?.[0] as Library;
  expect(sent.shape.pieces).toHaveLength(3);
});
