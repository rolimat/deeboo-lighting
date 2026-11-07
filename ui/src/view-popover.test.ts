import { beforeEach, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  api: {
    devPaintTest: vi.fn(() => Promise.resolve()),
    devMicLevel: vi.fn(() => Promise.resolve(-12.34)),
  },
}));
import type { Status } from "./api";
import { api } from "./api";
import { mountPopover } from "./view-popover";

let root: HTMLElement;
let view: ReturnType<typeof mountPopover>;

beforeEach(() => {
  document.body.innerHTML = '<div id="app"></div>';
  root = document.getElementById("app") as HTMLElement;
  view = mountPopover(root);
});

const strip = () => root.querySelector(".strip-status")?.textContent;
const status = (s: Status["strip"], debug = false): Status => ({
  strip: s,
  debug,
  clockTicks: null,
});
const testButton = () => root.querySelector<HTMLButtonElement>(".test-strip");

it("renders_header", () => {
  expect(root.querySelector(".header")?.textContent).toBe("Deeboo Lighting");
});

it("shows_not_connected_when_not_configured", () => {
  view.update(status({ kind: "notConfigured" }));
  expect(strip()).toBe("Strip · not connected");
});

it("shows_name_and_segments_when_connected", () => {
  view.update(status({ kind: "connected", name: "dj lights", segments: 10 }));
  expect(strip()).toBe("Strip · dj lights (10 segments)");
});

it("shows_connecting_and_error", () => {
  view.update(status({ kind: "connecting" }));
  expect(strip()).toBe("Strip · connecting…");
  view.update(status({ kind: "error" }));
  expect(strip()).toBe("Strip · connection error");
});

it("hides_test_strip_in_release", () => {
  view.update(status({ kind: "connected", name: "dj lights", segments: 10 }, false));
  expect(testButton()).toBeNull();
});

it("shows_test_strip_only_when_debug_and_connected", () => {
  view.update(status({ kind: "notConfigured" }, true));
  expect(testButton()).toBeNull();
  view.update(status({ kind: "connected", name: "dj lights", segments: 10 }, true));
  expect(testButton()?.textContent).toBe("Test strip");
});

it("test_strip_click_calls_the_backend", () => {
  view.update(status({ kind: "connected", name: "dj lights", segments: 10 }, true));
  testButton()?.click();
  expect(api.devPaintTest).toHaveBeenCalledOnce();
});

const clock = () => root.querySelector(".midi-clock")?.textContent;
const micButton = () => root.querySelector<HTMLButtonElement>(".mic-level");
const mic = () => root.querySelector(".mic-result")?.textContent;
const flush = () => new Promise((r) => setTimeout(r));

it("shows_midi_clock_ticks_only_when_present", () => {
  view.update({ ...status({ kind: "notConfigured" }, true), clockTicks: 48 });
  expect(clock()).toBe("MIDI clock · 48 ticks");
  view.update({ ...status({ kind: "notConfigured" }, true), clockTicks: null });
  expect(clock()).toBeUndefined();
});

it("hides_dev_probes_in_release", () => {
  view.update(status({ kind: "notConfigured" }, false));
  expect(micButton()).toBeNull();
});

it("mic_level_button_shows_dbfs_or_error", async () => {
  view.update(status({ kind: "notConfigured" }, true));
  expect(micButton()?.textContent).toBe("Mic level");
  micButton()?.click();
  await flush();
  expect(mic()).toBe("Mic · -12.3 dBFS");
  vi.mocked(api.devMicLevel).mockRejectedValueOnce("denied");
  micButton()?.click();
  await flush();
  expect(mic()).toBe("Mic · error");
});
