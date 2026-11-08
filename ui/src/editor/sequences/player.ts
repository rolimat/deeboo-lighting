import type { Step } from "../api";

const HALF_BEATS: Record<string, number> = { half: 1, one: 2, two: 4, four: 8 };

/**
 * The step the strip shows at `halfBeat` half-beats after Start. Mirrors `light_core::engine`
 * (`step_scene`): step 0 begins on Start, each step lasts its length in half-beats, the sequence
 * loops, and nothing restarts it on bar or phrase boundaries (only a moment change or a new Start).
 * The fixture `player-fixture.json` is generated from the engine and checked in both languages.
 */
export function stepAt(sequence: { steps: Pick<Step, "len">[] }, halfBeat: number): number {
  const lens = sequence.steps.map((s) => HALF_BEATS[s.len] ?? 1);
  const total = lens.reduce((a, b) => a + b, 0);
  if (total === 0) return 0;
  let left = Math.max(0, Math.floor(halfBeat)) % total;
  const i = lens.findIndex((l) => (left -= l) < 0);
  return Math.max(0, i);
}
