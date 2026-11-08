import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library, MomentSet } from "../api";

vi.mock("../api", () => ({
  api: { getLibrary: vi.fn(), saveLibrary: vi.fn(), previewGenerated: vi.fn() },
}));
import { api } from "../api";
import { SetEditor } from "./SetEditor";

const hsv = (h: number) => ({ h, s: 1000, v: 1000 });
const ms = (sequences: MomentSet["sequences"]): MomentSet => ({
  sequences,
  palettes: [1],
  order: "inOrder",
  interval: { bars: 2 },
});
const seq = (name: string, moment: string) => ({
  name,
  moment,
  steps: [{ len: "one", roles: [] }],
});
const lib = (): Library => ({
  schemaVersion: 1,
  shape: { pieces: [] },
  palettes: [[1, { name: "Test", principal: hsv(0), contraste: hsv(240), suave: hsv(120) }]],
  sequences: [
    [10, seq("Pulse", "rhythm")],
    [11, seq("Drift", "calm")],
    [12, seq("Rise", "buildUp")],
  ],
  sets: [
    {
      id: 1,
      name: "Main",
      rhythm: ms([{ id: 10 }, "generated"]),
      calm: ms([{ id: 11 }]),
      buildup: ms([{ id: 12 }]),
    },
    {
      id: 2,
      name: "Club",
      rhythm: ms(["generated"]),
      calm: ms(["generated"]),
      buildup: ms(["generated"]),
    },
  ],
  activeSet: 1,
  activePalette: 1,
});

let root: HTMLElement;
const settle = () => new Promise((r) => setTimeout(r));
const q = <T extends Element>(sel: string): T => {
  const el = root.querySelector<T>(sel);
  if (!el) throw new Error(`no ${sel}`);
  return el;
};
const group = (label: string) => q(`fieldset[aria-label='${label}']`);
const button = (label: string) => {
  const b = [...root.querySelectorAll("button")].find((x) => x.textContent === label);
  if (!b) throw new Error(`no button ${label}`);
  return b;
};
const change = (el: HTMLInputElement | HTMLSelectElement, v: string) =>
  act(() => {
    el.value = v;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  });
const click = (el: Element) =>
  act(async () => {
    (el as HTMLElement).click();
    await settle();
  });

beforeEach(async () => {
  root = document.createElement("div");
  document.body.append(root);
  vi.mocked(api.getLibrary).mockResolvedValue(lib());
  vi.mocked(api.saveLibrary).mockImplementation(async (l) => l);
  await act(async () => {
    render(<SetEditor />, root);
    await settle();
  });
});
afterEach(() => {
  render(null, root);
  root.remove();
  vi.clearAllMocks();
});

it("set_lists_only_matching_moment_sequences", () => {
  const names = (m: string) =>
    [...group(m).querySelectorAll("input[type=checkbox]")].map((c) => c.getAttribute("aria-label"));
  expect(names("Rhythm sequences")).toEqual(["Pulse", "Generated"]);
  expect(names("Calm sequences")).toEqual(["Drift", "Generated"]);
  expect(names("Build-up sequences")).toEqual(["Rise", "Generated"]);
});

it("saves_edits_and_keeps_generated", async () => {
  await change(q("select[aria-label='Rhythm order']"), "random");
  await change(q("select[aria-label='Rhythm interval unit']"), "phrases");
  await change(q("input[aria-label='Rhythm interval']"), "4");
  await click(button("Save"));
  const s = vi.mocked(api.saveLibrary).mock.calls[0]?.[0].sets[0];
  expect(s?.rhythm.order).toBe("random");
  expect(s?.rhythm.interval).toEqual({ phrases: 4 });
  expect(s?.rhythm.sequences).toEqual([{ id: 10 }, "generated"]);
});

it("interval_validation", async () => {
  for (const bad of ["0", "1000", ""]) {
    await change(q("input[aria-label='Calm interval']"), bad);
    await click(button("Save"));
    expect(api.saveLibrary).not.toHaveBeenCalled();
    expect(root.textContent).toContain("Interval must be a whole number from 1 to 999");
  }
});

it("shows_rust_message_on_refused_save", async () => {
  vi.mocked(api.saveLibrary).mockRejectedValue("cannot delete the last set");
  await click(button("Delete set"));
  await click(button("Save"));
  expect(root.textContent).toContain("cannot delete the last set");
});

it("save_keeps_the_popovers_active_set_unless_it_was_deleted", async () => {
  // The popover switched to set 2 after this section loaded.
  vi.mocked(api.getLibrary).mockResolvedValue({ ...lib(), activeSet: 2 });
  await click(button("Save"));
  expect(vi.mocked(api.saveLibrary).mock.calls[0]?.[0].activeSet).toBe(2);
  // Deleting the set the popover has active falls back to the first remaining one.
  await click(button("Club"));
  await click(button("Delete set"));
  await click(button("Save"));
  expect(vi.mocked(api.saveLibrary).mock.calls[1]?.[0].activeSet).toBe(1);
});

it("generated_entry_toggles", async () => {
  const gen = (m: string) =>
    q<HTMLInputElement>(`fieldset[aria-label='${m} sequences'] input[aria-label='Generated']`);
  expect(gen("Rhythm").checked).toBe(true);
  expect(gen("Calm").checked).toBe(false);
  await click(gen("Rhythm"));
  await click(gen("Calm"));
  await click(button("Save"));
  const s = vi.mocked(api.saveLibrary).mock.calls[0]?.[0].sets[0];
  expect(s?.rhythm.sequences).toEqual([{ id: 10 }]);
  expect(s?.calm.sequences).toEqual([{ id: 11 }, "generated"]);
});

it("preview_generated_shows_in_simulation", async () => {
  vi.mocked(api.previewGenerated).mockResolvedValue({
    name: "Generated",
    moment: "rhythm",
    steps: [{ len: "one", roles: [] }],
  });
  await click(q("button[aria-label='Preview generated Rhythm']"));
  expect(api.previewGenerated).toHaveBeenCalledWith("rhythm");
  expect(root.querySelector("svg[aria-label='Simulation']")).not.toBeNull();
});
