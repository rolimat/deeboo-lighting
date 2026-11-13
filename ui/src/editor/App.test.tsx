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
  },
}));
import { api, type WizardState } from "./api";
import { App } from "./App";

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
