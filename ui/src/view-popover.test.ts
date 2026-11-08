import { beforeEach, expect, it, vi } from "vitest";

vi.mock("./api", () => ({ api: {} }));
import type { Status } from "./api";
import { mountPopover } from "./view-popover";

let root: HTMLElement;
let view: ReturnType<typeof mountPopover>;

beforeEach(() => {
  document.body.innerHTML = '<div id="app"></div>';
  root = document.getElementById("app") as HTMLElement;
  view = mountPopover(root);
});

const strip = () => root.querySelector(".strip-status")?.textContent;
const status = (s: Status["strip"]): Status => ({
  strip: s,
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
});

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

it("has_no_dev_tools", () => {
  view.update(status({ kind: "connected", name: "dj lights", segments: 10 }));
  expect(root.querySelector("button")).toBeNull();
});
