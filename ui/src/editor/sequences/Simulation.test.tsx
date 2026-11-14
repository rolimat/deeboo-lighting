import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Palette, Piece, Sequence } from "../api";
import { Simulation } from "./Simulation";
import { roleColour } from "./RolePicker";
import type { Status } from "../../api";

vi.mock("../api", () => ({
  api: {
    getFullStatus: vi.fn(async () => ({ playing: false })),
    onStatus: vi.fn(async () => () => {}),
  },
}));
import { api } from "../api";

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
  fade: true,
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
const fills = () =>
  [...root.querySelectorAll("svg[aria-label=Simulation] rect")].map((r) => r.getAttribute("fill"));
const play = () =>
  act(() => {
    root.querySelector<HTMLElement>("button[aria-label='Play']")?.click();
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
afterEach(async () => {
  // act() flushes pending effects and cleanups before jsdom is torn down.
  await act(async () => render(null, root));
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
  expect(root.querySelector("svg[aria-label=Simulation]")).toBeNull();
});

it("has_no_bpm_control", async () => {
  await mount();
  expect(root.querySelector("input")).toBeNull();
});

it("follows_the_clock_when_it_plays_else_120", async () => {
  let push: (s: Partial<Status>) => void = () => {};
  vi.mocked(api.onStatus).mockImplementation(async (cb) => {
    push = (s) => cb(s as Status);
    return () => {};
  });
  await mount();
  await play();
  const t0 = performance.now();
  const tick = (ms: number) =>
    act(() => {
      const cbs = [...frames.values()];
      frames.clear();
      cbs.forEach((cb) => cb(t0 + ms));
    });
  await tick(0);
  // The clock says: bar 1, beat 2 (half-beat 2) at 60 BPM. Steps are 2 half-beats: step 1 shows.
  await act(() => push({ playing: true, bpm: 60, beatInBar: 2, barInPhrase: 1, phraseBars: 8 }));
  await tick(10);
  expect(fills()[1]).toBe(roleColour("principal", pal(0)));
  // 60 BPM: half a second per half-beat, so 1.2 s later we are at half-beat 4 -> step 0 again.
  await tick(1300);
  expect(fills()[0]).toBe(roleColour("principal", pal(0)));
});

it("a_16_bar_sequence_reaches_bars_9_to_16_while_the_clock_wraps_at_the_phrase", async () => {
  let push: (s: Partial<Status>) => void = () => {};
  vi.mocked(api.onStatus).mockImplementation(async (cb) => {
    push = (s) => cb(s as Status);
    return () => {};
  });
  const bar = (second: boolean) => ({
    len: "four" as const,
    roles: second ? (["off", "principal"] as const) : (["principal", "off"] as const),
  });
  const long: Sequence = {
    name: "L",
    moment: "rhythm",
    fade: true,
    steps: [...Array(8).fill(bar(false)), ...Array(8).fill(bar(true))],
  };
  await mount(pal(0), long);
  await play();
  const t0 = performance.now();
  const tick = (ms: number) =>
    act(() => {
      const cbs = [...frames.values()];
      frames.clear();
      cbs.forEach((cb) => cb(t0 + ms));
    });
  const clock = { playing: true, bpm: 120, beatInBar: 1, barInPhrase: 1, phraseBars: 8 };
  await act(() => push(clock));
  await tick(0);
  expect(fills()[0]).toBe(roleColour("principal", pal(0)));
  // 8 bars later (16 s at 120 BPM) the clock is at bar 1 of the phrase again: this is bar 9.
  await act(() => push(clock));
  await tick(16_020);
  expect(fills()[1]).toBe(roleColour("principal", pal(0)));
});

it("draws_a_fade_with_color_mix", async () => {
  const calm = (fade: boolean): Sequence => ({
    name: "C",
    moment: "calm",
    fade,
    steps: [
      { len: "four", roles: ["principal", "principal"] },
      { len: "four", roles: ["suave", "suave"] },
    ],
  });
  const run = async (s: Sequence) => {
    await mount(pal(0), s);
    await play();
    const t0 = performance.now();
    await act(() => {
      const cbs = [...frames.values()];
      frames.clear();
      cbs.forEach((cb) => cb(t0));
    });
    await act(() => {
      const cbs = [...frames.values()];
      frames.clear();
      cbs.forEach((cb) => cb(t0 + 2300)); // 120 BPM: 250 ms per half-beat, so half-beat 9
    });
  };
  await run(calm(true));
  expect(fills()[0]).toMatch(/^color-mix\(in srgb,/);
  expect(fills()[0]).toContain("50%");
  await act(async () => render(null, root));
  frames.clear();
  await run(calm(false));
  expect(fills()[0]).toBe(roleColour("suave", pal(0)));
});
