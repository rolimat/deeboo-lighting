import { afterEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";

vi.mock("./api", () => ({
  api: {
    getStatus: vi.fn(),
    getLibrary: vi.fn(async () => ({ shape: { pieces: [] } })),
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

afterEach(() => vi.restoreAllMocks());

it("renders_strip_section", async () => {
  vi.mocked(api.getStatus).mockResolvedValue({
    kind: "connected",
    name: "dj lights",
    segments: 10,
  });
  const root = document.createElement("div");
  await mount(root);
  const nav = root.querySelector("nav");
  expect(nav?.textContent).toBe("Strip");
  expect(nav?.querySelector("[aria-current='page']")?.textContent).toBe("Strip");
  expect(root.querySelector("section")?.textContent).toContain("dj lights (10 segments)");
});

it("shows_not_connected_when_no_strip", async () => {
  vi.mocked(api.getStatus).mockResolvedValue({ kind: "notConfigured" });
  const root = document.createElement("div");
  await mount(root);
  expect(root.querySelector("section")?.textContent).toContain("not connected");
});
