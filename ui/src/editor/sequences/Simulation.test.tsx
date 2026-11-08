import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Palette, Piece, Sequence } from "../api";
import { Simulation } from "./Simulation";
import { roleColour } from "./RolePicker";

const hsv = (h: number) => ({ h, s: 1000, v: 1000 });
const pal = (h: number): Palette => ({
  name: "P",
  principal: hsv(h),
  contraste: hsv(240),
  suave: hsv(120),
});
const pieces: Piece[] = [0, 1].map((i) => ({ x: i * 2, y: 0, angle: 0, len: 1 }));
const seq: Sequence = {
  name: "S",
  moment: "rhythm",
  steps: [
    { len: "one", roles: ["principal", "off"] },
    { len: "one", roles: ["off", "principal"] },
  ],
};

let root: HTMLElement;
let frames: Map<number, FrameRequestCallback>;
let next: number;
const setVisibility = (v: "visible" | "hidden") => {
  Object.defineProperty(document, "visibilityState", { value: v, configurable: true });
  act(() => {
    document.dispatchEvent(new Event("visibilitychange"));
  });
};
const mount = (p = pal(0), s = seq) =>
  act(() => {
    render(<Simulation sequence={s} pieces={pieces} palette={p} />, root);
  });
const fills = () => [...root.querySelectorAll("rect")].map((r) => r.getAttribute("fill"));
const play = () =>
  act(() => {
    [...root.querySelectorAll("button")].find((b) => b.textContent === "Play")?.click();
  });

beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
  frames = new Map();
  next = 1;
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => {
    frames.set(next, cb);
    return next++;
  });
  vi.spyOn(window, "cancelAnimationFrame").mockImplementation((id) => {
    frames.delete(id);
  });
  setVisibility("visible");
});
afterEach(() => {
  render(null, root);
  root.remove();
  vi.restoreAllMocks();
});

it("colours_follow_palette", async () => {
  await mount(pal(0));
  expect(fills()).toEqual([roleColour("principal", pal(0)), roleColour("off", pal(0))]);
  await mount(pal(200));
  expect(fills()[0]).toBe(roleColour("principal", pal(200)));
  expect(fills()[0]).not.toBe(roleColour("principal", pal(0)));
});

it("advances_with_the_beat_at_the_chosen_bpm", async () => {
  await mount();
  const t0 = performance.now();
  await play();
  const tick = (ms: number) =>
    act(() => {
      const cbs = [...frames.values()];
      frames.clear();
      cbs.forEach((cb) => cb(t0 + ms));
    });
  await tick(0);
  expect(fills()[0]).toBe(roleColour("principal", pal(0)));
  await tick(600); // 120 BPM: 250 ms per half-beat, steps are 2 half-beats: now at half-beat 2
  expect(fills()[1]).toBe(roleColour("principal", pal(0)));
});

it("stops_when_hidden", async () => {
  await mount();
  await play();
  expect(frames.size).toBe(1);
  setVisibility("hidden");
  expect(frames.size).toBe(0);
  expect(window.cancelAnimationFrame).toHaveBeenCalled();
  setVisibility("visible");
  expect(frames.size).toBe(1);
  await act(() => {
    render(null, root); // unmount while playing
  });
  expect(frames.size).toBe(0);
});

it("no_frames_while_paused", async () => {
  await mount();
  expect(window.requestAnimationFrame).not.toHaveBeenCalled();
});

it("shows_a_notice_when_pieces_and_roles_differ", async () => {
  await act(() => {
    render(<Simulation sequence={seq} pieces={pieces.slice(0, 1)} palette={pal(0)} />, root);
  });
  expect(root.textContent).toContain("does not match");
  expect(root.querySelector("svg")).toBeNull();
});

it("bpm_is_kept_between_60_and_180", async () => {
  await mount();
  const input = root.querySelector<HTMLInputElement>("input[aria-label='BPM']");
  if (!input) throw new Error("no bpm");
  for (const [typed, shown] of [
    ["500", "180"],
    ["10", "60"],
  ] as const) {
    input.value = typed;
    await act(() => {
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    expect(input.value).toBe(shown);
  }
});
