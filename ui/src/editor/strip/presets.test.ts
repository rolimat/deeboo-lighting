import { expect, it } from "vitest";
import { PRESETS, preset, type PresetName } from "./presets";
import { bounds, fits, endpoints } from "./shape";

it("presets_have_n_pieces_without_overlap", () => {
  expect(PRESETS.map((x) => x.label)).toEqual([
    "Line",
    "Two rows",
    "L shape",
    "U shape",
    "Arch",
    "Zig-zag",
    "Circle",
  ]);
  for (const { name } of PRESETS) {
    for (let n = 1; n <= 60; n++) {
      const ps = preset(name as PresetName, n);
      expect(ps, `${name} ${n}`).toHaveLength(n);
      expect(fits(ps), `${name} ${n} fits the grid`).toBe(true);
      const b = bounds(ps);
      expect(b.x0, `${name} ${n}`).toBeGreaterThanOrEqual(0);
      expect(b.y0, `${name} ${n}`).toBeGreaterThanOrEqual(0);
      expect(ps.every((q) => q.angle % 15 === 0 && q.angle < 360 && q.len === 1)).toBe(true);
      const mids = ps.map((q) => {
        const e = endpoints(q);
        return { x: (e.a.x + e.b.x) / 2, y: (e.a.y + e.b.y) / 2 };
      });
      for (let i = 0; i < n; i++)
        for (let j = i + 1; j < n; j++) {
          const [a, b] = [mids[i], mids[j]];
          const d = Math.hypot((a?.x ?? 0) - (b?.x ?? 0), (a?.y ?? 0) - (b?.y ?? 0));
          expect(d, `${name} ${n}: ${i + 1} vs ${j + 1}`).toBeGreaterThan(0.45);
        }
    }
  }
});

it("line_is_a_row_of_touching_cells", () => {
  expect(preset("line", 3)).toEqual([
    { x: 0, y: 0, angle: 0, len: 1 },
    { x: 1, y: 0, angle: 0, len: 1 },
    { x: 2, y: 0, angle: 0, len: 1 },
  ]);
});
