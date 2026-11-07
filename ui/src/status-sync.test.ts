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

const connecting: Status = { strip: { kind: "connecting" }, debug: true, clockTicks: null };
const connected: Status = {
  strip: { kind: "connected", name: "dj lights", segments: 10 },
  debug: true,
  clockTicks: null,
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
