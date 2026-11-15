import { expect, it } from "vitest";
import { fitLevel, fmtDb, tickStep, ticks, view, yPct } from "./zoom";

it("view_centres_on_ref_and_stays_inside_the_scale", () => {
  expect(view(-14, 0)).toEqual([0, -60]);
  expect(view(-14, 1)).toEqual([0, -30]);
  expect(view(-30, 2)).toEqual([-22.5, -37.5]);
  expect(view(-58, 3)).toEqual([-52.5, -60]);
});

it("tick_step_follows_the_span", () => {
  expect([60, 30, 15, 7.5].map(tickStep)).toEqual([10, 5, 2, 1]);
});

it("ticks_are_multiples_inside_the_view", () => {
  expect(ticks([-22.5, -37.5])).toEqual([-24, -26, -28, -30, -32, -34, -36]);
});

it("y_pct_is_percent_from_the_top", () => {
  expect(yPct(-15, [0, -30])).toBe(50);
  expect(yPct(5, [0, -30])).toBeLessThan(0);
});

it("fit_level_picks_the_closest_view_showing_everything", () => {
  expect(fitLevel(-30, [-31, -29, -23, -42])).toBe(1);
  expect(fitLevel(-30, [-31, -29])).toBe(3);
  expect(fitLevel(-30, [-1, -59])).toBe(0);
});

it("fmt_db_keeps_one_decimal", () => {
  expect(fmtDb(7)).toBe("7.0");
  expect(fmtDb(14.3)).toBe("14.3");
});
