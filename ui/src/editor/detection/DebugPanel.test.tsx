import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { render } from "preact";
import type { DetectorEvent } from "../api";

const flag = vi.hoisted(() => ({ on: false }));
vi.mock("./debug", () => ({
  get DEBUG() {
    return flag.on;
  },
}));
import { Timeline } from "./Timeline";

const beat = (over: Partial<DetectorEvent> = {}): DetectorEvent => ({
  kickDb: -20,
  subDb: -30,
  highDb: -25,
  kickRefDb: -20,
  subRefDb: -30,
  highRefDb: -25,
  moment: "rhythm",
  cause: null,
  progress: {
    highBarDb: null,
    riserBars: 0,
    riserNeedBars: 4,
    riserRiseDb: 0,
    riserNeedDb: 6,
    approachCalmBars: 0,
    approachNeedBars: 4,
    approachAboveDb: 0,
    approachRiseDb: 0,
    approachNeedRiseDb: 2,
    levelOn: false,
    levelAboveDb: null,
  },
  ...over,
});
const ring = (): DetectorEvent[] => [
  beat({ progress: { ...beat().progress, highBarDb: -24 } }),
  beat({ moment: "buildUp", cause: "riser" }),
  beat({
    moment: "buildUp",
    progress: { ...beat().progress, highBarDb: -22, riserBars: 2, riserRiseDb: 1.2 },
  }),
];
const t = { calmKickDb: 14, calmHighDb: 10, riserDb: 6 };

let root: HTMLElement;
const show = () => render(<Timeline ring={ring()} t={t} shift={0} onChange={() => {}} />, root);
beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
});
afterEach(() => {
  render(null, root);
  root.remove();
});

it("flag_off_shows_nothing_new", () => {
  flag.on = false;
  show();
  expect(root.querySelector("[data-debug]")).toBeNull();
  expect(root.querySelector("[data-bar-steps]")).toBeNull();
});

it("flag_on_shows_cause_panel_and_bar_steps", () => {
  flag.on = true;
  show();
  expect(root.querySelectorAll("[data-moment-run]")[1]?.textContent).toMatch(/Build-up\s*· riser/);
  const panel = root.querySelector("[data-debug=panel]")?.textContent ?? "";
  expect(panel).toContain("Riser 2/4 bars, +1.2 of 6.0 dB");
  expect(panel).toContain("✗");
  expect(panel).not.toContain("Level");
  expect(root.querySelector("[data-bar-steps]")).not.toBeNull();
});
