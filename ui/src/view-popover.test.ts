import { beforeEach, expect, it, vi } from "vitest";

vi.mock("./api", () => ({ api: { devPaintTest: vi.fn(() => Promise.resolve()) } }));
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
const status = (s: Status["strip"], debug = false): Status => ({ strip: s, debug });
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
