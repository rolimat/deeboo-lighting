import type { Library } from "../api";

/** Client handle → id Rust assigned, per kind. */
export interface IdMap {
  sets: Map<number, number>;
  palettes: Map<number, number>;
  sequences: Map<number, number>;
}

/** Rust renumbers new entries in place, so entries at the same index are the same entry. */
function pair<T>(sent: T[], saved: T[], id: (t: T) => number): Map<number, number> {
  const m = new Map<number, number>();
  if (sent.length !== saved.length) return m;
  sent.forEach((t, i) => {
    const a = id(t);
    const b = id(saved[i] as T);
    if (a !== b) m.set(a, b);
  });
  return m;
}

export function diffIds(sent: Library, saved: Library): IdMap {
  return {
    sets: pair(sent.sets, saved.sets, (s) => s.id),
    palettes: pair(sent.palettes, saved.palettes, (p) => p[0]),
    sequences: pair(sent.sequences, saved.sequences, (s) => s[0]),
  };
}

export const isEmpty = (m: IdMap) => !m.sets.size && !m.palettes.size && !m.sequences.size;

/** Rewrites every id and reference in a library (history states) through the map. */
export function remapLibrary(l: Library, m: IdMap): Library {
  const p = (id: number) => m.palettes.get(id) ?? id;
  const q = (id: number) => m.sequences.get(id) ?? id;
  const moment = <T extends { sequences: Library["sets"][number]["rhythm"]["sequences"] }>(
    x: T,
  ): T => ({
    ...x,
    sequences: x.sequences.map((r) => (typeof r === "object" && "id" in r ? { id: q(r.id) } : r)),
  });
  return {
    ...l,
    palettes: l.palettes.map(([id, v]) => [p(id), v]),
    sequences: l.sequences.map(([id, v]) => [q(id), v]),
    sets: l.sets.map((s) => ({
      ...s,
      id: m.sets.get(s.id) ?? s.id,
      palettes: s.palettes.map(p),
      rhythm: moment(s.rhythm),
      calm: moment(s.calm),
      buildup: moment(s.buildup),
    })),
    activeSet: m.sets.get(l.activeSet) ?? l.activeSet,
    activePalette: l.activePalette === null ? null : p(l.activePalette),
  };
}
