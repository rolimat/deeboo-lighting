import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Library } from "../api";

vi.mock("../api", () => ({ api: { getLibrary: vi.fn(), saveLibrary: vi.fn() } }));
import { api } from "../api";
import { PaletteEditor } from "./PaletteEditor";

const hsv = (h: number) => ({ h, s: 1000, v: 1000 });
const lib = (): Library => ({
  schemaVersion: 1,
  shape: { pieces: [] },
  palettes: [
    [1, { name: "Test", principal: hsv(0), contraste: hsv(240), suave: hsv(120) }],
    [2, { name: "Other", principal: hsv(10), contraste: hsv(20), suave: hsv(30) }],
  ],
  sequences: [],
  sets: [],
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
const button = (label: string) => {
  const b = [...root.querySelectorAll("button")].find((x) => x.textContent === label);
  if (!b) throw new Error(`no button ${label}`);
  return b;
};
const setValue = (el: HTMLInputElement, v: string) =>
  act(() => {
    el.value = v;
    el.dispatchEvent(new Event("input", { bubbles: true }));
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
    render(<PaletteEditor />, root);
    await settle();
  });
});
afterEach(() => {
  render(null, root);
  root.remove();
  vi.clearAllMocks();
});

it("palette_edits_save", async () => {
  await setValue(q("input[aria-label='Palette name']"), "Warm");
  await setValue(q("input[aria-label='Principal hue']"), "45");
  await setValue(q("input[aria-label='Suave value']"), "900");
  await click(button("Save"));
  const p = vi.mocked(api.saveLibrary).mock.calls[0]?.[0].palettes[0]?.[1];
  expect(p?.name).toBe("Warm");
  expect(p?.principal.h).toBe(45);
  expect(p?.suave.v).toBe(900);
  expect(root.textContent).toContain("use a light colour for Calm");
});

it("rejects_bad_name_without_saving", async () => {
  await setValue(q("input[aria-label='Palette name']"), "");
  await click(button("Save"));
  expect(api.saveLibrary).not.toHaveBeenCalled();
  expect(root.textContent).toContain("Name must be 1 to 63 characters");
});

it("delete_palette_in_use_is_refused", async () => {
  vi.mocked(api.saveLibrary).mockRejectedValue("palette 2 is used by set 1");
  await click(button("Other"));
  await click(button("Delete palette"));
  await click(button("Save"));
  expect(root.textContent).toContain("palette 2 is used by set 1");
});
