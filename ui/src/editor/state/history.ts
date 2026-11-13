export interface History<T> {
  readonly present: T;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  push(next: T): void;
  /** Overwrites the present without a new undo step (gestures, server-assigned ids). */
  replace(next: T): void;
  /** Rewrites every undo and redo state (not the present) through `fn`. */
  remapStates(fn: (v: T) => T): void;
  undo(): void;
  redo(): void;
}

/** Linear undo stack of whole values, at most `limit` steps back. */
export function createHistory<T>(initial: T, limit = 100): History<T> {
  let past: T[] = [];
  let future: T[] = [];
  let present = initial;
  return {
    get present() {
      return present;
    },
    get canUndo() {
      return past.length > 0;
    },
    get canRedo() {
      return future.length > 0;
    },
    push(next) {
      past = [...past, present].slice(-limit);
      future = [];
      present = next;
    },
    replace(next) {
      present = next;
    },
    remapStates(fn) {
      past = past.map(fn);
      future = future.map(fn);
    },
    undo() {
      const prev = past.at(-1);
      if (prev === undefined) return;
      future = [present, ...future];
      past = past.slice(0, -1);
      present = prev;
    },
    redo() {
      const next = future[0];
      if (next === undefined) return;
      past = [...past, present];
      future = future.slice(1);
      present = next;
    },
  };
}
