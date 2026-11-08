import type { Piece } from "../api";

/** The library's coordinate grid, 0..=GRID in cells (mirrors `light_core::library::GRID`). */
export const GRID = 64;

export type Angle = Piece["angle"];
export interface Rect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** Nearest grid cell, kept on the grid. */
export function snap(v: number): number {
  return Number.isFinite(v) ? Math.min(GRID, Math.max(0, Math.round(v))) : 0;
}

export function rotate(a: Angle): Angle {
  return ((a + 90) % 360) as Angle;
}

/** A piece is `len` cells by 1, turned clockwise about its anchor (y grows downwards, like SVG). */
function rect(p: Piece): Rect {
  switch (p.angle) {
    case 0:
      return { x0: p.x, y0: p.y, x1: p.x + p.len, y1: p.y + 1 };
    case 90:
      return { x0: p.x - 1, y0: p.y, x1: p.x, y1: p.y + p.len };
    case 180:
      return { x0: p.x - p.len, y0: p.y - 1, x1: p.x, y1: p.y };
    case 270:
      return { x0: p.x, y0: p.y - p.len, x1: p.x + 1, y1: p.y };
  }
}

/** The smallest rectangle holding every piece. */
export function bounds(pieces: Piece[]): Rect {
  const rs = pieces.map(rect);
  if (rs.length === 0) return { x0: 0, y0: 0, x1: 1, y1: 1 };
  return {
    x0: Math.min(...rs.map((r) => r.x0)),
    y0: Math.min(...rs.map((r) => r.y0)),
    x1: Math.max(...rs.map((r) => r.x1)),
    y1: Math.max(...rs.map((r) => r.y1)),
  };
}

/** Index of the topmost piece (the last drawn) under the point, or -1. */
export function hitTest(pieces: Piece[], x: number, y: number): number {
  for (let i = pieces.length - 1; i >= 0; i--) {
    const p = pieces[i];
    if (!p) continue;
    const r = rect(p);
    if (x >= r.x0 && x <= r.x1 && y >= r.y0 && y <= r.y1) return i;
  }
  return -1;
}
