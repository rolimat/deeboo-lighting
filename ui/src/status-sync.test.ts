import { beforeEach, expect, it, vi } from "vitest";

import type { Status } from "./api";

let push: ((s: Status) => void) | undefined;
let resolveGet: ((s: Status) => void) | undefined;

vi.mock("./api", () => ({
  api: {
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
