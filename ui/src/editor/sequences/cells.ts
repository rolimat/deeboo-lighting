import type { Step } from "../api";

/** One column per half-beat, one role per segment: `cols[halfBeat][segment]`. */
export type Cols = string[][];

export const BAR = 8; // half-beats
export const MAX_BARS = 16;
const LENS: [string, number][] = [
  ["four", 8],
  ["two", 4],
  ["one", 2],
  ["half", 1],
];
const HALF_BEATS: Record<string, number> = Object.fromEntries(LENS);

export function toCells(steps: Step[]): Cols {
  return steps.flatMap((s) => Array.from({ length: HALF_BEATS[s.len] ?? 1 }, () => s.roles));
}

const same = (a: string[], b: string[]) => a.length === b.length && a.every((r, i) => r === b[i]);

/** Merges equal consecutive columns into the longest steps that do not cross a bar line. */
export function fromCells(cols: Cols): Step[] {
  const steps: Step[] = [];
  for (let i = 0; i < cols.length;) {
    const col = cols[i] as string[];
    let run = 1;
    while (i + run < cols.length && same(cols[i + run] as string[], col)) run++;
    const [len, n] = LENS.find(([, n]) => n <= run && (i % BAR) + n <= BAR) as [string, number];
    steps.push({ len, roles: [...col] });
    i += n;
  }
  return steps;
}

/** Whole bars only: adding copies the last bar, removing drops the end; 1 to 16 bars. */
export function setBars(cols: Cols, bars: number): Cols {
  const want = Math.min(MAX_BARS, Math.max(1, Math.round(bars))) * BAR;
  if (want <= cols.length) return cols.slice(0, want);
  const last = cols.slice(-BAR);
  const out = [...cols];
  while (out.length < want) out.push(...last.slice(0, want - out.length));
  return out;
}
