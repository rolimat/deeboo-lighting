import { beforeEach, expect, it, vi } from "vitest";

import type { Status } from "./api";

let push: ((s: Status) => void) | undefined;
let resolveGet: ((s: Status) => void) | undefined;

vi.mock("./api", () => ({
  api: {
    popoverVisible: vi.fn(() => Promise.resolve()),
    onStatus: vi.fn((cb: (s: Status) => void) => {
      push = cb;
      return Promise.resolve(() => {});
    }),
    getStatus: vi.fn(
      () =>
        new Promise<Status>((r) => {
          resolveGet = r;
        }),
    ),
  },
}));
import { api } from "./api";
import { syncStatus } from "./status-sync";

const base: Omit<Status, "strip"> = {
  bpm: null,
  playing: false,
  beatInBar: null,
  barInPhrase: null,
  phraseBars: 8,
  moment: "rhythm",
  auto: true,
  predicted: false,
  lightsOn: true,
  midi: { source: null, ok: true },
  audio: { ok: false },
  clip: null,
  selection: {
    set: "Default",
    setId: 1,
    paletteAuto: false,
    palette: {
      name: "Test",
      colours: [
        { h: 0, s: 1000, v: 1000 },
        { h: 240, s: 1000, v: 1000 },
        { h: 120, s: 350, v: 1000 },
      ],
    },
  },
};
const connecting: Status = { ...base, strip: { kind: "connecting" } };
const connected: Status = {
  ...base,
  strip: { kind: "connected", name: "dj lights", segments: 10, reported: 10 },
};

beforeEach(() => {
  push = undefined;
  resolveGet = undefined;
});

it("applies_the_initial_status", async () => {
  const update = vi.fn();
  const done = syncStatus(update);
  await vi.waitFor(() => expect(resolveGet).toBeDefined());
  resolveGet?.(connecting);
  await done;
  expect(update).toHaveBeenLastCalledWith(connecting);
});

it("applies_pushed_status_changes", async () => {
  const update = vi.fn();
  const done = syncStatus(update);
  await vi.waitFor(() => expect(resolveGet).toBeDefined());
  resolveGet?.(connecting);
  await done;
  push?.(connected);
  expect(update).toHaveBeenLastCalledWith(connected);
});

it("a_push_before_the_initial_read_returns_wins", async () => {
  const update = vi.fn();
  const done = syncStatus(update);
  await vi.waitFor(() => expect(resolveGet).toBeDefined());
  push?.(connected);
  resolveGet?.(connecting);
  await done;
  expect(update).toHaveBeenLastCalledWith(connected);
});

function setVisibility(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { value: state, configurable: true });
  document.dispatchEvent(new Event("visibilitychange"));
}

it("reports_its_own_visibility_to_the_backend", async () => {
  vi.mocked(api.popoverVisible).mockClear();
  const done = syncStatus(vi.fn());
  await vi.waitFor(() => expect(resolveGet).toBeDefined());
  resolveGet?.(connecting);
  await done;
  setVisibility("visible");
  expect(api.popoverVisible).toHaveBeenLastCalledWith(true);
  setVisibility("hidden");
  expect(api.popoverVisible).toHaveBeenLastCalledWith(false);
  window.dispatchEvent(new Event("blur"));
  expect(api.popoverVisible).toHaveBeenLastCalledWith(false);
});

it("rereads_the_status_each_time_it_becomes_visible", async () => {
  const update = vi.fn();
  const done = syncStatus(update);
  await vi.waitFor(() => expect(resolveGet).toBeDefined());
  resolveGet?.(connecting);
  await done;
  resolveGet = undefined;
  setVisibility("visible");
  await vi.waitFor(() => expect(resolveGet).toBeDefined());
  (resolveGet as ((s: Status) => void) | undefined)?.(connected);
  await vi.waitFor(() => expect(update).toHaveBeenLastCalledWith(connected));
});
