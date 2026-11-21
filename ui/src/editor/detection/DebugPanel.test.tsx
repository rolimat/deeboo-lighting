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
import { CAUSE_WORDS } from "./causes";

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
    riserNearRef: true,
    approachCalmBars: 0,
    approachNeedBars: 4,
    approachAboveDb: 0,
    approachNeedAboveDb: 2,
    approachRiseDb: 0,
    approachNeedRiseDb: 2,
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
  expect(root.querySelectorAll("[data-moment-run]")[1]?.textContent).toMatch(
    /Build-up\s*· highs climbed/,
  );
  const panel = root.querySelector("[data-debug=panel]")?.textContent ?? "";
  expect(panel).toContain("Riser 2/4 bars, +1.2 of 6.0 dB");
  expect(panel).toContain("✗");
  expect(root.querySelector("[data-bar-steps]")).not.toBeNull();
});

const panelText = (progress: Partial<DetectorEvent["progress"]>) => {
  flag.on = true;
  render(
    <Timeline
      ring={[beat({ progress: { ...beat().progress, ...progress } })]}
      t={t}
      shift={0}
      onChange={() => {}}
    />,
    root,
  );
  const rows = [...root.querySelectorAll("[data-debug=panel] div")].map((d) => d.textContent ?? "");
  return {
    riser: rows.find((r) => r.includes("Riser")) ?? "",
    approach: rows.find((r) => r.includes("Approach")) ?? "",
  };
};
const riserOk = { riserBars: 3, riserNeedBars: 3, riserRiseDb: 6, riserNearRef: true };
const approachOk = {
  approachCalmBars: 8,
  approachNeedBars: 8,
  approachAboveDb: 2.5,
  approachRiseDb: 2,
};

it("riser_row_is_checked_only_when_bars_rise_and_near_ref_all_hold", () => {
  expect(panelText(riserOk).riser).toMatch(/^✓/);
  const far = panelText({ ...riserOk, riserNearRef: false }).riser;
  expect(far).toMatch(/^✗/);
  expect(far).toContain("near ref ✗");
  expect(panelText({ ...riserOk, riserBars: 2 }).riser).toMatch(/^✗/);
  expect(panelText({ ...riserOk, riserRiseDb: 5.9 }).riser).toMatch(/^✗/);
});

it("approach_row_is_checked_only_when_calm_bars_above_ref_and_rise_all_hold", () => {
  expect(panelText(approachOk).approach).toMatch(/^✓/);
  expect(panelText({ ...approachOk, approachCalmBars: 7 }).approach).toMatch(/^✗/);
  expect(panelText({ ...approachOk, approachAboveDb: 1.9 }).approach).toMatch(/^✗/);
  expect(panelText({ ...approachOk, approachRiseDb: 1.9 }).approach).toMatch(/^✗/);
});

it("every_cause_has_plain_english_words", () => {
  expect(CAUSE_WORDS).toEqual({
    riser: "highs climbed",
    approach: "slow build",
    quiet: "kick gone",
    teaser: "short tease",
    groove: "groove build-up",
    kickDrop: "kick back",
    hatsDrop: "hats back",
    slowReturn: "slow return",
    collapse: "build-up faded",
    timeout: "too long",
    softBreak: "soft break",
    phraseStart: "phrase start",
    undo: "undone",
  });
});
