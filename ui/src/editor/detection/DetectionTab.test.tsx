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
  expect(text).toContain("Calm when 14 dB below");
  expect(text).toContain("Calm when 10 dB below");
  expect(text).toContain("Build-up when 6 dB above");
  expect(line("Kick drop").getAttribute("aria-valuenow")).toBe("14");
});

it("threshold_labels_sit_in_a_column_beside_the_waveform", async () => {
  await mount();
  push(beat());
  await act(async () => void (await vi.advanceTimersByTimeAsync(0)));
  const cols = root.querySelectorAll<HTMLElement>("[data-readouts]");
  expect(cols.length).toBe(3);
  for (const lane of root.querySelectorAll("[data-lane]"))
    expect(lane.textContent).not.toMatch(/ dB /);
  const high = cols[2];
  expect(high?.textContent).toContain("Build-up when 6 dB above");
  expect(high?.textContent).toContain("Calm when");
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
