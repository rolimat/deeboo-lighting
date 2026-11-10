import type { Piece } from "../api";

/** The library's coordinate grid, 0..=GRID in cells (mirrors `light_core::library::GRID`). */
export const GRID = 64;
/** An end within this many cells of a neighbour's end hooks onto it. */
const MAGNET = 0.6;

export interface Rect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}
export interface Pt {
  x: number;
  y: number;
}

/** Nearest grid cell. */
export function snap(v: number): number {
  return Number.isFinite(v) ? Math.round(v) : 0;
}

/** `angle` plus `delta` degrees, kept in 0..360. */
export function turn(angle: number, delta: number): number {
  return (((angle + delta) % 360) + 360) % 360;
}

/** Unit vectors along and across a piece (exact for the right angles). */
function axes(angle: number): { u: Pt; v: Pt } {
  const a = turn(angle, 0);
  const [c, s] =
    a % 90 === 0
      ? ([
          [1, 0],
          [0, 1],
          [-1, 0],
          [0, -1],
        ][a / 90] as [number, number])
      : [Math.cos((a * Math.PI) / 180), Math.sin((a * Math.PI) / 180)];
  return { u: { x: c, y: s }, v: { x: -s, y: c } };
}

/** A piece is `len` cells by 1, turned clockwise about its anchor (y grows downwards, like SVG). */
function corners(p: Piece): Pt[] {
  const { u, v } = axes(p.angle);
  const at = (a: number, b: number) => ({
    x: p.x + a * u.x + b * v.x,
    y: p.y + a * u.y + b * v.y,
  });
  return [at(0, 0), at(p.len, 0), at(p.len, 1), at(0, 1)];
}

/** Both ends of the piece's centre line, where neighbours meet. */
export function endpoints(p: Piece): { a: Pt; b: Pt } {
  const { u, v } = axes(p.angle);
  const a = { x: p.x + 0.5 * v.x, y: p.y + 0.5 * v.y };
  return { a, b: { x: a.x + p.len * u.x, y: a.y + p.len * u.y } };
}

/** The smallest rectangle holding every piece. */
export function bounds(pieces: Piece[]): Rect {
  const pts = pieces.flatMap(corners);
  if (pts.length === 0) return { x0: 0, y0: 0, x1: 1, y1: 1 };
  const xs = pts.map((c) => c.x);
  const ys = pts.map((c) => c.y);
  return { x0: Math.min(...xs), y0: Math.min(...ys), x1: Math.max(...xs), y1: Math.max(...ys) };
}

const move = (ps: Piece[], dx: number, dy: number): Piece[] =>
  ps.map((p) => ({ ...p, x: p.x + dx, y: p.y + dy }));

/** Moves every piece by whole cells so none reaches below 0: the canvas grows to the top and left. */
export function normalise(pieces: Piece[]): Piece[] {
  const b = bounds(pieces);
  const dx = b.x0 < 0 ? Math.ceil(-b.x0) : 0;
  const dy = b.y0 < 0 ? Math.ceil(-b.y0) : 0;
  return dx === 0 && dy === 0 ? pieces : move(pieces, dx, dy);
}

/** The library accepts anchors on 0..=GRID. */
export function fits(pieces: Piece[]): boolean {
  return pieces.every((p) => p.x >= 0 && p.y >= 0 && p.x <= GRID && p.y <= GRID);
}

/** Where piece `i` anchors when dragged to the (already grid-snapped) `x`,`y`: an end near a neighbour's end hooks on. */
export function magnet(pieces: Piece[], i: number, x: number, y: number): Pt {
  const p = pieces[i];
  if (!p) return { x, y };
  const mine = endpoints({ ...p, x, y });
  let best: (Pt & { d: number }) | null = null;
  pieces.forEach((q, j) => {
    if (j === i) return;
    const theirs = endpoints(q);
    for (const e of [mine.a, mine.b])
      for (const f of [theirs.a, theirs.b]) {
        const d = Math.hypot(f.x - e.x, f.y - e.y);
        if (d < MAGNET && (!best || d < best.d)) best = { x: f.x - e.x, y: f.y - e.y, d };
      }
  });
  const m = best as (Pt & { d: number }) | null;
  return m ? { x: x + m.x, y: y + m.y } : { x, y };
}

/** The piece turned to `angle` about the start of its centre line, so it stays hooked to what it touches. */
export function spin(p: Piece, angle: number): Piece {
  const { a } = endpoints(p);
  const { v } = axes(angle);
  return { ...p, angle, x: a.x - 0.5 * v.x, y: a.y - 0.5 * v.y };
}
