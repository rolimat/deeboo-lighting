import type { Library } from "../api";
import { remapLibrary, type IdMap } from "./ids";

/** A `saveLibrary` mock that renumbers unknown ids to max+1 like Rust's `assign_ids`. */
export function renumbering(initial: Library) {
  let prev = initial;
  return async (l: Library): Promise<Library> => {
    const assign = <T>(known: number[], items: T[], id: (t: T) => number) => {
      let next = Math.max(0, ...known) + 1;
      const m = new Map<number, number>();
      for (const t of items) if (!known.includes(id(t))) m.set(id(t), next++);
      return m;
    };
    const map: IdMap = {
      sets: assign(
        prev.sets.map((s) => s.id),
        l.sets,
        (s) => s.id,
      ),
      palettes: assign(
        prev.palettes.map((p) => p[0]),
        l.palettes,
        (p) => p[0],
      ),
      sequences: assign(
        prev.sequences.map((p) => p[0]),
        l.sequences,
        (p) => p[0],
      ),
    };
    prev = remapLibrary(l, map);
    return prev;
  };
}
