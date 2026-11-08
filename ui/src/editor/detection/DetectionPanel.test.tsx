import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Settings } from "../../api";
import type { DetectorEvent } from "../api";

vi.mock("../api", () => ({
  api: {
    getSettings: vi.fn(),
    detectionWatch: vi.fn(async () => {}),
    setDetectionThresholds: vi.fn(async () => ({})),
    onDetector: vi.fn(),
  },
}));
import { api } from "../api";
import { DetectionPanel } from "./DetectionPanel";

const settings = (over: Partial<Settings> = {}): Settings => ({
  midiSource: null,
  audioInput: null,
  phraseBars: 8,
  sensitivity: 5,
  brightnessPct: 100,
  leadMs: 100,
  lightsOn: true,
  calmKickDb: 20,
  calmHighDb: 10,
  riserDb: 5,
  ...over,
});

let root: HTMLElement;
let push: (e: DetectorEvent) => void = () => {};
const unlisten = vi.fn();
const settle = () => act(async () => void (await Promise.resolve()));
const slider = (label: string) => {
  const el = root.querySelector<HTMLInputElement>(`input[aria-label='${label}']`);
  if (!el) throw new Error(`no ${label}`);
  return el;
};
const move = (label: string, v: string) =>
  act(() => {
    const el = slider(label);
    el.value = v;
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });

beforeEach(async () => {
  vi.useFakeTimers();
  root = document.createElement("div");
  document.body.append(root);
  vi.mocked(api.getSettings).mockResolvedValue(settings());
  vi.mocked(api.onDetector).mockImplementation(async (cb) => {
    push = cb;
    return unlisten;
  });
  await act(async () => {
    render(<DetectionPanel />, root);
    await vi.advanceTimersByTimeAsync(0);
  });
});
afterEach(() => {
  render(null, root);
  root.remove();
  vi.clearAllMocks();
  vi.useRealTimers();
});

it("watches_while_mounted", async () => {
  expect(api.detectionWatch).toHaveBeenCalledWith(true);
  await act(async () => render(null, root));
  expect(api.detectionWatch).toHaveBeenLastCalledWith(false);
  expect(unlisten).toHaveBeenCalled();
});

it("shows_meters_and_moment", async () => {
  push({
    kickDb: -20,
    subDb: -30,
    highDb: -25,
    kickRefDb: -18,
    subRefDb: -28,
    highRefDb: -24,
    moment: "calm",
  });
  await settle();
  expect(root.querySelectorAll("[role=meter]").length).toBe(3);
  expect(root.textContent).toContain("Calm");
});

it("sliders_send_thresholds", async () => {
  expect(slider("Kick drop").value).toBe("20");
  await move("Kick drop", "25");
  await move("Kick drop", "26");
  expect(api.setDetectionThresholds).not.toHaveBeenCalled();
  await act(async () => void (await vi.advanceTimersByTimeAsync(400)));
  expect(api.setDetectionThresholds).toHaveBeenCalledTimes(1);
  expect(api.setDetectionThresholds).toHaveBeenCalledWith({
    calmKickDb: 26,
    calmHighDb: 10,
    riserDb: 5,
  });
});

it("reset_restores_defaults", async () => {
  const reset = [...root.querySelectorAll("button")].find((b) => b.textContent === "Reset");
  await act(async () => {
    reset?.click();
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(api.setDetectionThresholds).toHaveBeenCalledWith({
    calmKickDb: 16,
    calmHighDb: 12,
    riserDb: 7,
  });
  expect(slider("Kick drop").value).toBe("16");
  expect(slider("Highs drop").value).toBe("12");
  expect(slider("Riser").value).toBe("7");
});

it("unmounting_right_after_a_move_still_saves_it", async () => {
  await move("Kick drop", "27");
  await act(async () => render(null, root));
  expect(api.setDetectionThresholds).toHaveBeenCalledTimes(1);
  expect(api.setDetectionThresholds).toHaveBeenCalledWith({
    calmKickDb: 27,
    calmHighDb: 10,
    riserDb: 5,
  });
  await act(async () => void (await vi.advanceTimersByTimeAsync(400)));
  expect(api.setDetectionThresholds).toHaveBeenCalledTimes(1);
});
