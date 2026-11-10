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
import { api } from "./api";
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
  });
  const root = document.createElement("div");
  await mount(root);
  const tab = root.querySelector("[role=tab][aria-selected=true]");
  expect(tab?.textContent).toBe("Strip");
  expect(root.querySelector("section")?.textContent).toContain("dj lights (10 segments)");
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
