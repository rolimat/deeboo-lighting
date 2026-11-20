export const ZOOMS = [1, 2, 4, 8] as const;
export const FLOOR_DB = 60;

/** [top, bottom] dB of a lane at this zoom level: span 60/zoom centred on ref, kept inside -60..0. */
export function view(ref: number, level: number): [number, number] {
  const span = FLOOR_DB / (ZOOMS[level] ?? 1);
  const top = Math.min(0, Math.max(-FLOOR_DB + span, ref + span / 2));
  return [top, top - span];
}

/** Midpoint of the reference and its threshold lines: the view centres here so every line sits inside it. */
export function centreOf(ref: number, lines: number[]): number {
  const all = [ref, ...lines];
  return (Math.min(...all) + Math.max(...all)) / 2;
}

/** Percent from the top (may be <0 or >100 when outside the view). */
export const yPct = (db: number, v: [number, number]): number =>
  ((v[0] - db) / (v[0] - v[1])) * 100;

/** Scale step for a span in dB. */
export const tickStep = (span: number): number =>
  span >= 60 ? 10 : span >= 30 ? 5 : span >= 15 ? 2 : 1;

/** The dB marks to draw for a view (multiples of tickStep strictly inside it). */
export function ticks(v: [number, number]): number[] {
  const step = tickStep(v[0] - v[1]);
  const out: number[] = [];
  for (let t = Math.floor(v[0] / step) * step; t > v[1]; t -= step) if (t < v[0]) out.push(t);
  return out;
}

/** Highest level (3 → 0) where every value sits inside 2%..98% of the view; 0 if none. */
export function fitLevel(ref: number, values: number[]): number {
  for (let l = ZOOMS.length - 1; l > 0; l--) {
    const v = view(ref, l);
    if (values.every((x) => yPct(x, v) >= 2 && yPct(x, v) <= 98)) return l;
  }
  return 0;
}

/** One decimal, e.g. 14.3 → "14.3", 7 → "7.0". */
export const fmtDb = (db: number): string => db.toFixed(1);
