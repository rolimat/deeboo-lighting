import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Settings, Status } from "../../api";
import type { DetectorEvent } from "../api";

vi.mock("../api", () => ({
  api: {
    getSettings: vi.fn(),
    saveSettings: vi.fn(),
    getFullStatus: vi.fn(),
    onStatus: vi.fn(),
    detectionWatch: vi.fn(async () => {}),
    setDetectionThresholds: vi.fn(async () => ({})),
    onDetector: vi.fn(),
  },
}));
import { api } from "../api";
import { DetectionTab } from "./DetectionTab";

const settings = (over: Partial<Settings> = {}): Settings => ({
  midiSource: null,
  audioInput: null,
  phraseBars: 8,
  sensitivity: 5,
  brightnessPct: 100,
  leadMs: 100,
  lightsOn: true,
  calmKickDb: 14,
  calmHighDb: 10,
  helpTooltips: true,
  showInDock: "editor",
  detectorByInput: {},
  riserDb: 6,
  ...over,
});
const hsv = { h: 0, s: 0, v: 0 };
const status = (over: Partial<Status> = {}): Status => ({
  strip: { kind: "notFound" },
  bpm: 122,
  playing: true,
  beatInBar: 1,
  barInPhrase: 1,
  phraseBars: 8,
  moment: "rhythm",
  auto: true,
  predicted: false,
  lightsOn: true,
  midi: { source: "Clock A", ok: true },
  audio: { ok: true, device: "Mic B", levelDb: -20 },
  selection: {
    set: "",
    setId: 1,
    paletteAuto: true,
    palette: { name: "", colours: [hsv, hsv, hsv] },
  },
  clip: null,
  ...over,
});
const beat = (over: Partial<DetectorEvent> = {}): DetectorEvent => ({
  kickDb: -20,
  subDb: -30,
  highDb: -25,
  kickRefDb: -20,
  subRefDb: -30,
  highRefDb: -25,
  moment: "rhythm",
  ...over,
});

let root: HTMLElement;
let push: (e: DetectorEvent) => void = () => {};
const unlisten = vi.fn();
const mount = async (st: Status = status(), goSetup = vi.fn()) => {
  vi.mocked(api.getFullStatus).mockResolvedValue(st);
  await act(async () => {
    render(<DetectionTab onGoSetup={goSetup} />, root);
    await vi.advanceTimersByTimeAsync(0);
  });
  return goSetup;
};
const line = (name: string) => {
  const el = root.querySelector<HTMLElement>(`[role=slider][aria-label='${name}']`);
  if (!el) throw new Error(`no ${name}`);
  return el;
};
const mouse = (type: string, clientY: number) =>
  window.dispatchEvent(new MouseEvent(type, { clientY, bubbles: true }));

beforeEach(() => {
  vi.useFakeTimers();
  root = document.createElement("div");
  document.body.append(root);
  vi.mocked(api.getSettings).mockResolvedValue(settings());
  vi.mocked(api.saveSettings).mockImplementation(async (s) => s);
  vi.mocked(api.onStatus).mockResolvedValue(unlisten);
  vi.mocked(api.onDetector).mockImplementation(async (cb) => {
    push = cb;
    return unlisten;
  });
});
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
  vi.clearAllMocks();
  vi.useRealTimers();
});

it("watches_while_mounted", async () => {
  await mount();
  expect(api.detectionWatch).toHaveBeenCalledWith(true);
  await act(async () => render(null, root));
  expect(api.detectionWatch).toHaveBeenLastCalledWith(false);
  expect(unlisten).toHaveBeenCalled();
});

it("axis_reads_last_8_bars_now", async () => {
  await mount();
  const text = root.textContent ?? "";
  expect(text).toContain("Last 8 bars");
  expect(text).toContain("now ▸");
  expect(root.querySelectorAll("[data-bar]").length).toBe(8);
});

it("threshold_label_in_words", async () => {
  await mount();
  push(beat());
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
  const text = root.textContent ?? "";
  expect(text).toContain("Calm when 14.0 dB below");
  expect(text).toContain("Calm when 10.0 dB below");
  expect(text).toContain("Build-up when 6.0 dB above");
  expect(line("Kick drop").getAttribute("aria-valuenow")).toBe("14");
});

it("calm_lines_show_the_sensitivity_the_detector_applies", async () => {
  // Sensitivity 9 lowers the calm thresholds by 4 dB; the riser is not shifted.
  vi.mocked(api.getSettings).mockResolvedValue(settings({ sensitivity: 9 }));
  await mount();
  push(beat());
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
  const text = root.textContent ?? "";
  expect(text).toContain("Calm when 10.0 dB below");
  expect(text).toContain("Calm when 6.0 dB below");
  expect(text).toContain("Build-up when 6.0 dB above");
  expect(line("Kick drop").getAttribute("aria-valuenow")).toBe("10");
  await act(async () => {
    line("Kick drop").dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
    );
    await vi.advanceTimersByTimeAsync(400);
  });
  // Shown 11 dB means 15 dB stored.
  expect(api.setDetectionThresholds).toHaveBeenCalledWith({
    calmKickDb: 15,
    calmHighDb: 10,
    riserDb: 6,
  });
});

it("threshold_labels_sit_on_their_line_inside_the_lane", async () => {
  await mount();
  push(beat());
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
  expect(root.querySelector("[data-readouts]")).toBeNull();
  const high = root.querySelector("[data-lane='high']")?.textContent ?? "";
  expect(high).toContain("Build-up when 6.0 dB above");
  expect(high).toContain("Calm when 10.0 dB below");
});

const knobs = (lane: string) => root.querySelectorAll(`[data-knobs='${lane}'] [role=slider]`);
const knob = (name: string) => {
  const el = root.querySelector<HTMLElement>(`[role=slider][aria-label='${name}']`);
  if (!el) throw new Error(`no ${name}`);
  return el;
};
const lane = async (n = 1) => {
  await mount();
  for (let i = 0; i < n; i++) push(beat());
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
};
const click = async (label: string, id: string) => {
  const b = root.querySelector<HTMLButtonElement>(
    `[data-lane-row='${id}'] button[aria-label='${label}']`,
  );
  if (!b) throw new Error(`no ${label}`);
  await act(async () => b.click());
  return b;
};
const scale = (id: string) =>
  [...root.querySelectorAll(`[data-lane='${id}'] [data-scale]`)].map((e) => Number(e.textContent));

it("knob_column_has_one_knob_per_threshold", async () => {
  await lane();
  expect(knobs("kick").length).toBe(1);
  expect(knobs("sub").length).toBe(0);
  expect(knobs("high").length).toBe(2);
});

it("knob_arrow_changes_threshold_and_saves", async () => {
  await lane();
  await act(async () => {
    knob("Kick drop knob").dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
    );
    await vi.advanceTimersByTimeAsync(400);
  });
  expect(api.setDetectionThresholds).toHaveBeenLastCalledWith({
    calmKickDb: 15,
    calmHighDb: 10,
    riserDb: 6,
  });
  await act(async () => {
    knob("Kick drop knob").dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", shiftKey: true, bubbles: true }),
    );
    await vi.advanceTimersByTimeAsync(400);
  });
  expect(api.setDetectionThresholds).toHaveBeenLastCalledWith({
    calmKickDb: 15.1,
    calmHighDb: 10,
    riserDb: 6,
  });
});

it("zoom_in_shows_finer_scale", async () => {
  await lane();
  expect(scale("sub").length).toBeGreaterThan(0);
  expect(scale("sub").every((n) => n % 10 === 0)).toBe(true);
  // Numbers are centred on their mark.
  expect(root.querySelector("[data-lane='high'] [data-scale-num]")?.className).toContain(
    "-translate-y-1/2",
  );
  await click("Zoom in", "sub");
  await click("Zoom in", "sub");
  const fine = scale("sub");
  expect(fine.length).toBeGreaterThan(2);
  expect(fine.every((n) => n % 2 === 0)).toBe(true);
  expect(fine.some((n) => n % 10 !== 0)).toBe(true);
  await click("Zoom in", "sub");
  expect((await click("Zoom in", "sub")).disabled).toBe(true);
});

it("threshold_outside_view_pins_its_label_and_click_zooms_out", async () => {
  await lane();
  await click("Zoom in", "kick");
  await click("Zoom in", "kick");
  await click("Zoom in", "kick");
  // Kick ref -20, view -23.75..-16.25 at x8; the line at -34 is below it.
  const pinned = root.querySelector<HTMLButtonElement>("[data-lane='kick'] button");
  expect(pinned?.textContent).toContain("▼");
  expect(pinned?.textContent).toContain("Calm when 14.0 dB below");
  // The slider stays mounted at the edge so keyboard focus is never lost.
  expect(line("Kick drop").style.top).toBe("100%");
  await act(async () => pinned?.click());
  expect(root.querySelector("[data-lane='kick'] button")).toBeNull();
  expect(root.querySelector("[data-lane='kick'] [data-label]")).not.toBeNull();
});

it("fit_picks_a_closer_view", async () => {
  await lane();
  await click("Zoom to fit", "high");
  // Highs ref -25, thresholds at -19 and -37: x2 (step 5) is the closest view still holding both.
  expect(scale("high").every((n) => n % 5 === 0)).toBe(true);
  expect(scale("high").some((n) => n % 10 !== 0)).toBe(true);
  await click("Zoom to fit", "sub");
  // Sub has no thresholds and a flat signal: fits tight.
  expect(scale("sub").some((n) => n % 10 !== 0)).toBe(true);
});

it("drag_label_changes_value", async () => {
  await lane();
  const box = root.querySelector<HTMLElement>("[data-lane='kick']");
  vi.spyOn(box as HTMLElement, "getBoundingClientRect").mockReturnValue({
    top: 0,
    height: 100,
  } as DOMRect);
  const label = root.querySelector<HTMLElement>("[data-lane='kick'] [data-label]");
  await act(() => {
    label?.dispatchEvent(new MouseEvent("pointerdown", { clientY: 50, bubbles: true }));
    window.dispatchEvent(
      new MouseEvent("pointermove", { clientY: 70, shiftKey: true, bubbles: true }),
    );
  });
  await act(async () => {
    mouse("pointerup", 70);
    await vi.advanceTimersByTimeAsync(400);
  });
  // y 70 = -42 dB; reference -20: 22 below.
  expect(api.setDetectionThresholds).toHaveBeenLastCalledWith({
    calmKickDb: 22,
    calmHighDb: 10,
    riserDb: 6,
  });
});

it("shift_drag_snaps_to_tenths", async () => {
  await lane();
  const box = root.querySelector<HTMLElement>("[data-lane='kick']");
  vi.spyOn(box as HTMLElement, "getBoundingClientRect").mockReturnValue({
    top: 0,
    height: 100,
  } as DOMRect);
  await act(() => {
    line("Kick drop").dispatchEvent(new MouseEvent("pointerdown", { clientY: 60, bubbles: true }));
    window.dispatchEvent(
      new MouseEvent("pointermove", { clientY: 70.4, shiftKey: true, bubbles: true }),
    );
  });
  await act(async () => {
    mouse("pointerup", 70.4);
    await vi.advanceTimersByTimeAsync(400);
  });
  expect(api.setDetectionThresholds).toHaveBeenLastCalledWith({
    calmKickDb: 22.2,
    calmHighDb: 10,
    riserDb: 6,
  });
});

it("moment_lane_shows_what_the_show_used", async () => {
  await mount();
  push(beat());
  push(beat({ moment: "calm" }));
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
  const runs = [...root.querySelectorAll("[data-moment-run]")].map((e) => e.textContent);
  expect(runs).toEqual(["Rhythm", "Calm"]);
  expect(root.querySelector("[data-moment-run][data-now]")?.textContent).toBe("Calm");
});

it("drag_threshold_saves_db_in_range", async () => {
  await mount();
  push(beat());
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
  const lane = root.querySelector<HTMLElement>("[data-lane='kick']");
  // 100 px tall lane, 0 dB at the top, -60 dB at the bottom; reference -20 dB.
  vi.spyOn(lane as HTMLElement, "getBoundingClientRect").mockReturnValue({
    top: 0,
    height: 100,
  } as DOMRect);
  await act(() => {
    line("Kick drop").dispatchEvent(new MouseEvent("pointerdown", { clientY: 60, bubbles: true }));
    mouse("pointermove", 60); // -36 dB: 16 below the reference
    mouse("pointermove", 70); // -42 dB: 22 below
  });
  expect(api.setDetectionThresholds).not.toHaveBeenCalled();
  await act(async () => {
    mouse("pointerup", 70);
    await vi.advanceTimersByTimeAsync(400);
  });
  expect(api.setDetectionThresholds).toHaveBeenCalledTimes(1);
  expect(api.setDetectionThresholds).toHaveBeenCalledWith({
    calmKickDb: 22,
    calmHighDb: 10,
    riserDb: 6,
  });
  // Far below the lane: clamped to the Rust maximum (30).
  await act(async () => {
    line("Kick drop").dispatchEvent(new MouseEvent("pointerdown", { clientY: 70, bubbles: true }));
    mouse("pointermove", 100);
    mouse("pointerup", 100);
    await vi.advanceTimersByTimeAsync(400);
  });
  expect(api.setDetectionThresholds).toHaveBeenLastCalledWith({
    calmKickDb: 30,
    calmHighDb: 10,
    riserDb: 6,
  });
});

it("arrow_keys_move_a_threshold_by_one_db", async () => {
  await mount();
  await act(async () => {
    line("Riser").dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }));
    await vi.advanceTimersByTimeAsync(400);
  });
  expect(api.setDetectionThresholds).toHaveBeenCalledWith({
    calmKickDb: 14,
    calmHighDb: 10,
    riserDb: 7,
  });
});

it("shows_the_active_inputs_values", async () => {
  vi.mocked(api.getSettings).mockResolvedValue(
    settings({ audioInput: "Mic B", sensitivity: 9, calmKickDb: 22 }),
  );
  await mount();
  expect(root.querySelector<HTMLInputElement>("input[type=range]")?.value).toBe("4");
  expect(line("Kick drop").getAttribute("aria-valuenow")).toBe("18"); // 22 stored, minus the sensitivity 9 shift
});

it("reset_to_defaults_is_one_call", async () => {
  await mount();
  const reset = [...root.querySelectorAll("button")].find(
    (b) => b.textContent === "Reset to defaults",
  );
  await act(async () => {
    reset?.click();
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(api.saveSettings).toHaveBeenCalledTimes(1);
  expect(api.saveSettings).toHaveBeenCalledWith(
    settings({ sensitivity: 5, calmKickDb: 16, calmHighDb: 12, riserDb: 7 }),
  );
  expect(line("Kick drop").getAttribute("aria-valuenow")).toBe("16");
});

it("reset_keeps_settings_changed_elsewhere_since_mount", async () => {
  await mount();
  vi.mocked(api.getSettings).mockResolvedValue(
    settings({ lightsOn: false, brightnessPct: 30, helpTooltips: false }),
  );
  const reset = [...root.querySelectorAll("button")].find(
    (b) => b.textContent === "Reset to defaults",
  );
  await act(async () => {
    reset?.click();
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(api.saveSettings).toHaveBeenLastCalledWith(
    settings({
      lightsOn: false,
      brightnessPct: 30,
      helpTooltips: false,
      sensitivity: 5,
      calmKickDb: 16,
      calmHighDb: 12,
      riserDb: 7,
    }),
  );
});

it("changes_slider_saves_sensitivity", async () => {
  await mount();
  const input = root.querySelector<HTMLInputElement>("input[type=range]");
  await act(async () => {
    if (input) {
      input.value = "4";
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(api.saveSettings).toHaveBeenCalledWith(settings({ sensitivity: 9 }));
});

it("status_bar_greys_missing_clock_or_audio", async () => {
  await mount(status({ audio: { ok: false } }));
  const bar = root.querySelector("[data-status-bar]");
  expect(bar?.querySelector("[data-clock]")?.className).not.toContain("text-dim");
  expect(bar?.textContent).toContain("MIDI clock playing");
  expect(bar?.textContent).toContain("from Clock A");
  const audio = bar?.querySelector("[data-audio]");
  expect(audio?.className).toContain("text-dim");
  expect(audio?.textContent).toContain("No audio");
  expect(audio?.textContent).toContain("Setup");
  expect(bar?.textContent).toContain("Live view: it moves in real time");
});

it("status_bar_says_when_the_capture_failed", async () => {
  await mount(status({ audio: { ok: false, failed: true } }));
  const audio = root.querySelector("[data-status-bar] [data-audio]");
  expect(audio?.textContent).toContain("Capture failed");
  expect(audio?.className).toContain("text-red");
});

it("status_bar_shows_listening_device", async () => {
  await mount();
  const bar = root.querySelector("[data-status-bar]");
  expect(bar?.textContent).toContain("Listening");
  expect(bar?.textContent).toContain("Mic B");
});

it("empty_state_waiting_for_clock_generic_copy", async () => {
  const goSetup = await mount(status({ playing: false, bpm: null }));
  const text = root.textContent ?? "";
  expect(text).toContain("Waiting for MIDI clock");
  expect(text).toContain(
    "Start playback in your clock source (DJ software, drum machine…). Detection follows the beats it sends.",
  );
  expect(text).not.toContain("MIDI Sync");
  expect(text).toContain("Clock A");
  expect(text).toContain("Mic B");
  expect(root.querySelector("[data-lane]")).toBeNull();
  const link = [...root.querySelectorAll("button")].find(
    (b) => b.textContent === "Change in Setup",
  );
  link?.click();
  expect(goSetup).toHaveBeenCalled();
});

it("forced_state_message", async () => {
  await mount(status({ auto: false }));
  expect(root.textContent).toContain("Moment forced — switch to Auto to see detection");
  expect(root.querySelector("[data-lane]")).toBeNull();
});

it("status_bar_is_above_the_moment_changes_slider", async () => {
  await mount();
  const bar = root.querySelector("[data-status-bar]");
  const slider = [...root.querySelectorAll("span")].find((e) => e.textContent === "Moment changes");
  expect(bar?.compareDocumentPosition(slider as Node)).toBeGreaterThanOrEqual(
    Node.DOCUMENT_POSITION_FOLLOWING,
  );
});

const press = (el: HTMLElement, k: string, shiftKey = false) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent("keydown", { key: k, shiftKey, bubbles: true }));
  });

it("line_arrows_keep_the_tenths", async () => {
  vi.mocked(api.getSettings).mockResolvedValue(settings({ calmKickDb: 14.3 }));
  await lane();
  await press(line("Kick drop"), "ArrowUp");
  expect(line("Kick drop").getAttribute("aria-valuenow")).toBe("15.3");
  await press(line("Kick drop"), "ArrowDown");
  await press(line("Kick drop"), "ArrowDown");
  expect(line("Kick drop").getAttribute("aria-valuenow")).toBe("13.3");
});

it("at_zoom_1_a_line_below_the_floor_stays_at_the_edge_and_draggable", async () => {
  vi.mocked(api.getSettings).mockResolvedValue(settings({ calmKickDb: 20 }));
  await mount();
  push(beat({ kickDb: -45, kickRefDb: -45 }));
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
  // -45 - 20 = -65: below the -60 floor, but at x1 it is clamped, never pinned.
  expect(root.querySelector("[data-lane='kick'] button")).toBeNull();
  expect(line("Kick drop").style.top).toBe("100%");
});

it("a_focused_line_that_leaves_the_view_keeps_focus_and_keys", async () => {
  await lane();
  await click("Zoom in", "kick");
  const el = line("Kick drop");
  await act(async () => el.focus());
  await click("Zoom in", "kick");
  // x4 spans 15 dB, centred between ref and line: 14 dB below is inside, 17 is out.
  for (let i = 0; i < 3; i++) await press(el, "ArrowUp");
  const pinned = root.querySelector("[data-lane='kick'] button");
  expect(pinned?.textContent).toContain("▼");
  expect(line("Kick drop")).toBe(el);
  expect(document.activeElement).toBe(el);
  expect(el.style.top).toBe("100%");
  await press(el, "ArrowUp");
  expect(el.getAttribute("aria-valuenow")).toBe("18");
});

it("highs_opens_at_x4_with_both_lines_inside_the_view", async () => {
  vi.mocked(api.getSettings).mockResolvedValue(settings({ riserDb: 3, calmHighDb: 10 }));
  await lane();
  expect(root.querySelector("[data-lane-row='high']")?.textContent).toContain("×4");
  expect(scale("high").every((n) => n % 2 === 0)).toBe(true);
  expect(root.querySelector("[data-lane='high'] button")).toBeNull();
  expect(root.querySelector("[data-lane-row='kick']")?.textContent).toContain("×1");
});

it("riser_knob_goes_down_to_1", async () => {
  vi.mocked(api.getSettings).mockResolvedValue(settings({ riserDb: 1 }));
  await lane();
  await press(knob("Riser knob"), "ArrowDown");
  expect(knob("Riser knob").getAttribute("aria-valuenow")).toBe("1");
});

it("the_view_holds_still_while_a_line_is_dragged", async () => {
  await lane();
  const box = root.querySelector<HTMLElement>("[data-lane='high']");
  vi.spyOn(box as HTMLElement, "getBoundingClientRect").mockReturnValue({
    top: 0,
    height: 100,
  } as DOMRect);
  const marks = () =>
    [...root.querySelectorAll("[data-lane='high'] [data-scale]")].map((e) => e.textContent);
  await act(() => {
    line("Riser").dispatchEvent(new MouseEvent("pointerdown", { clientY: 10, bubbles: true }));
    mouse("pointermove", 10);
  });
  const during = marks();
  await act(async () => {
    mouse("pointermove", 30);
  });
  expect(marks()).toEqual(during);
  await act(async () => {
    mouse("pointerup", 30);
  });
  expect(marks().length).toBeGreaterThan(0);
});

it("highs_opens_at_the_highest_level_where_both_lines_fit", async () => {
  vi.mocked(api.getSettings).mockResolvedValue(settings({ riserDb: 7, calmHighDb: 12 }));
  await lane();
  // 19 dB apart: x4 (15 dB) cannot hold both, x2 can.
  expect(root.querySelector("[data-lane-row='high']")?.textContent).toContain("×2");
  expect(root.querySelector("[data-lane='high'] button")).toBeNull();
});

const highBox = () => {
  const box = root.querySelector<HTMLElement>("[data-lane='high']");
  vi.spyOn(box as HTMLElement, "getBoundingClientRect").mockReturnValue({
    top: 0,
    height: 100,
  } as DOMRect);
};

it("pointercancel_ends_the_drag", async () => {
  await lane();
  highBox();
  const marks = () =>
    [...root.querySelectorAll("[data-lane='high'] [data-scale]")].map((e) => e.textContent);
  await act(() => {
    line("Riser").dispatchEvent(new MouseEvent("pointerdown", { clientY: 10, bubbles: true }));
    mouse("pointermove", 2);
  });
  const during = marks();
  const val = line("Riser").getAttribute("aria-valuenow");
  await act(async () => {
    mouse("pointercancel", 2);
  });
  expect(marks()).not.toEqual(during);
  await act(async () => {
    mouse("pointermove", 40);
  });
  expect(line("Riser").getAttribute("aria-valuenow")).toBe(val);
});

it("the_pointer_keeps_meaning_the_same_dB_when_the_reference_moves_mid_drag", async () => {
  await lane();
  highBox();
  await act(() => {
    line("Riser").dispatchEvent(new MouseEvent("pointerdown", { clientY: 20, bubbles: true }));
  });
  push(beat({ highDb: -15, highRefDb: -15 }));
  await act(async () => void (await vi.advanceTimersByTimeAsync(100)));
  await act(async () => {
    mouse("pointermove", 30);
  });
  const top = parseFloat(line("Riser").style.top);
  expect(Math.abs(top - 30)).toBeLessThan(2);
});
