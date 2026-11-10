import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import { api, type Library } from "../api";
import { createHistory, type History } from "./history";

export type SaveState =
  { state: "saving" } | { state: "saved" } | { state: "error"; reason: string };

const DEBOUNCE_MS = 400;
/** Edits with the same `coalesce` key closer than this are one gesture (one undo step). */
const GESTURE_MS = 1000;

/**
 * The editor's library: loads once, applies edits with undo/redo, and saves 400 ms after the last
 * change. The live selection (activeSet/activePalette) is never taken from history: every save
 * carries the selection the app last reported, so undo cannot change the set playing.
 */
export function useLibrary() {
  const hist = useRef<History<Library> | null>(null);
  const live = useRef<Pick<Library, "activeSet" | "activePalette"> | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const gesture = useRef<{ key: string; at: number } | null>(null);
  const rev = useRef(0);
  const [, render] = useState(0);
  const [save, setSave] = useState<SaveState>({ state: "saved" });
  const bump = () => render((n) => n + 1);

  const withLive = (l: Library): Library => ({ ...l, ...live.current });

  const flush = useCallback(async () => {
    const h = hist.current;
    if (!h) return;
    const sent = rev.current;
    try {
      const saved = await api.saveLibrary(withLive(h.present));
      live.current = { activeSet: saved.activeSet, activePalette: saved.activePalette };
      // Adopt server-assigned ids unless the user edited meanwhile.
      if (rev.current === sent) h.replace(saved);
      if (rev.current === sent) setSave({ state: "saved" });
      bump();
    } catch (e) {
      setSave({ state: "error", reason: String(e) });
    }
  }, []);

  const schedule = useCallback(() => {
    rev.current += 1;
    setSave({ state: "saving" });
    clearTimeout(timer.current);
    timer.current = setTimeout(() => void flush(), DEBOUNCE_MS);
    bump();
  }, [flush]);

  useEffect(() => {
    let alive = true;
    api.getLibrary().then(
      (l) => {
        if (!alive) return;
        live.current = { activeSet: l.activeSet, activePalette: l.activePalette };
        hist.current = createHistory(l);
        bump();
      },
      (e) => alive && setSave({ state: "error", reason: String(e) }),
    );
    return () => {
      alive = false;
      clearTimeout(timer.current);
    };
  }, []);

  const edit = useCallback(
    (fn: (l: Library) => Library, opts?: { coalesce?: string }) => {
      const h = hist.current;
      if (!h) return;
      const next = withLive(fn(h.present));
      const now = Date.now();
      const g = gesture.current;
      if (opts?.coalesce && g?.key === opts.coalesce && now - g.at < GESTURE_MS) h.replace(next);
      else h.push(next);
      gesture.current = opts?.coalesce ? { key: opts.coalesce, at: now } : null;
      schedule();
    },
    [schedule],
  );

  /** Re-reads which set and palette are live after a command changed them outside the library. */
  const syncLive = useCallback(async () => {
    const l = await api.getLibrary();
    live.current = { activeSet: l.activeSet, activePalette: l.activePalette };
    bump();
  }, []);

  /** Adopts a library Rust already saved (a command's result): no save, and the undo history starts over. */
  const reset = useCallback((l: Library) => {
    clearTimeout(timer.current);
    rev.current += 1;
    hist.current = createHistory({ ...l, ...live.current });
    gesture.current = null;
    setSave({ state: "saved" });
    bump();
  }, []);

  const undo = useCallback(() => {
    if (!hist.current?.canUndo) return;
    hist.current.undo();
    gesture.current = null;
    schedule();
  }, [schedule]);
  const redo = useCallback(() => {
    if (!hist.current?.canRedo) return;
    hist.current.redo();
    gesture.current = null;
    schedule();
  }, [schedule]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "z") return;
      if (t && /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) return; // native text undo
      e.preventDefault();
      if (e.shiftKey) redo();
      else undo();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [undo, redo]);

  return {
    lib: hist.current?.present ?? null,
    edit,
    reset,
    undo,
    redo,
    canUndo: hist.current?.canUndo ?? false,
    canRedo: hist.current?.canRedo ?? false,
    save,
    liveSet: live.current?.activeSet ?? null,
    syncLive,
  };
}
