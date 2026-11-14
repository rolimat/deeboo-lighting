import type { Hsv } from "../api";
import { expect, it } from "vitest";
import { fromHex, fromWheel, pastelPalette, toHex, wheelPoint } from "./colour";

it("wheel_maps_hue_and_saturation", () => {
  // hue = angle clockwise from the east (screen y points down), saturation = distance from the centre
  expect(fromWheel(1, 0, 700)).toEqual({ h: 0, s: 1000, v: 700 });
  expect(fromWheel(0, 0.5, 1000)).toEqual({ h: 90, s: 500, v: 1000 });
  expect(fromWheel(-1, 0, 1000)).toMatchObject({ h: 180, s: 1000 });
  expect(fromWheel(0, -1, 1000)).toMatchObject({ h: 270, s: 1000 });
  expect(fromWheel(0, 0, 1000).s).toBe(0);
  expect(fromWheel(3, 0, 1000).s).toBe(1000); // outside the disc clamps to the edge
  const p = wheelPoint({ h: 90, s: 500, v: 1000 });
  expect(p.x).toBeCloseTo(0);
  expect(p.y).toBeCloseTo(0.5);
});

it("hex_round_trips_and_rejects_invalid", () => {
  expect(toHex({ h: 0, s: 1000, v: 1000 })).toBe("#FF0000");
  expect(fromHex("#00ff00")).toEqual({ h: 120, s: 1000, v: 1000 });
  for (const hex of ["#FF0000", "#00FF00", "#0000FF", "#FFFFFF", "#000000", "#808080", "#FFE0B2"]) {
    expect(toHex(fromHex(hex) as Hsv)).toBe(hex);
  }
  for (const bad of ["", "#12", "#GGGGGG", "ff00", "#FF00000", "red"])
    expect(fromHex(bad)).toBeNull();
  expect(fromHex("FF2D55")).not.toBeNull(); // the # is optional
});

it("pastel_palette_rule", () => {
  const p = pastelPalette(340);
  expect(p.contraste.h).toBe(20);
  expect(p.suave.h).toBe(0);
  expect(pastelPalette(100)).toEqual({
    name: "New palette",
    principal: { h: 100, s: 600, v: 1000 },
    contraste: { h: 140, s: 600, v: 1000 },
    suave: { h: 120, s: 250, v: 1000 },
  });
  expect(pastelPalette(-20).principal.h).toBe(340);
});
