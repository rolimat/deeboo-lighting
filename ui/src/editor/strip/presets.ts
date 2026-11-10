import type { Piece } from "../api";
import { normalise, turn } from "./shape";

export type PresetName = "line" | "twoRows" | "lShape" | "uShape" | "arch" | "zigZag" | "circle";

/** Icon paths on a 44 x 24 box (stroke only). */
export const PRESETS: { name: PresetName; label: string; icon: string }[] = [
  { name: "line", label: "Line", icon: "M4 12h36" },
  { name: "twoRows", label: "Two rows", icon: "M4 7h36M4 17h36" },
  { name: "lShape", label: "L shape", icon: "M6 4v16h32" },
  { name: "uShape", label: "U shape", icon: "M6 4v16h32V4" },
  { name: "arch", label: "Arch", icon: "M4 20Q22-6 40 20" },
  { name: "zigZag", label: "Zig-zag", icon: "M4 18l9-12 9 12 9-12 9 12" },
  { name: "circle", label: "Circle", icon: "M22 3a9 9 0 1 0 .01 0" },
];

const snap15 = (deg: number) => turn(Math.round(deg / 15) * 15, 0);

/**
 * `n` one-cell pieces, each starting where the previous one ends, heading `angle(i)` degrees. Places
 * follow the exact angles and each piece is then turned to the nearest 15, so a closed curve stays closed.
 */
function chain(n: number, angle: (i: number) => number): Piece[] {
  const out: Piece[] = [];
  let x = 0; // start of the next piece's centre line
  let y = 0.5;
  for (let i = 0; i < n; i++) {
    const a = snap15(angle(i));
    const r = (a * Math.PI) / 180;
    // The anchor sits half a cell across from the centre line's start.
    out.push({ x: x + 0.5 * Math.sin(r), y: y - 0.5 * Math.cos(r), angle: a, len: 1 });
    const exact = (angle(i) * Math.PI) / 180;
    x += Math.cos(exact);
    y += Math.sin(exact);
  }
  return out.map((p) => ({ ...p, x: round(p.x), y: round(p.y) }));
}
const round = (v: number) => Math.round(v * 1000) / 1000;

/** The starting layout of `n` segments; every piece is one cell and nothing overlaps. */
export function preset(name: PresetName, n: number): Piece[] {
  const half = Math.ceil(n / 2);
  let ps: Piece[];
  switch (name) {
    case "line":
      ps = chain(n, () => 0);
      break;
    case "twoRows": {
      const top = chain(half, () => 0);
      const bottom = chain(n - half, () => 0).map((p) => ({ ...p, y: p.y + 2 }));
      ps = [...top, ...bottom];
      break;
    }
    case "lShape":
      ps = chain(n, (i) => (i < half ? 90 : 0));
      break;
    case "uShape": {
      const up = Math.floor(n / 3);
      const down = n > 1 ? Math.max(up, 1) : 0;
      ps = chain(n, (i) => (i < down ? 90 : i >= n - up ? 270 : 0));
      break;
    }
    case "arch":
      ps = chain(n, (i) => -90 + (180 * (i + 0.5)) / n);
      break;
    case "zigZag":
      ps = chain(n, (i) => (i % 2 === 0 ? 315 : 45));
      break;
    case "circle":
      ps = chain(n, (i) => (n === 2 ? 90 * i : (360 * i) / n)); // two pieces cannot close a loop
      break;
  }
  return normalise(ps);
}
