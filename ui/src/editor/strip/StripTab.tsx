import { useEffect, useRef, useState } from "preact/hooks";
import { api, type Piece } from "../api";
import { useHelpOn } from "../shell/Help";
import type { useLibrary } from "../state/library";
import { Guide } from "./Guide";
import { PRESETS, preset } from "./presets";
import { SegmentsCard, type StripInfo } from "./SegmentsCard";
import { ShapeCanvas } from "./ShapeCanvas";

/** Selecting a piece lights it on the strip; the last selection within this time wins. */
const IDENTIFY_DEBOUNCE_MS = 150;

export function StripTab({
  store,
  strip,
  onRepair,
  onWired,
}: {
  store: ReturnType<typeof useLibrary>;
  strip: StripInfo;
  /** Reconnect / Change strip: back into the wizard. */
  onRepair: () => void;
  /** The wired count changed: the strip status is out of date. */
  onWired: () => void;
}) {
  const [wired, setWired] = useState(strip.segments);
  const [selected, setSelected] = useState<number | null>(null);
  const [lit, setLit] = useState<number | null>(null);
  const [confirm, setConfirm] = useState<number | null>(null);
  const [error, setError] = useState("");
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const helpOn = useHelpOn();
  const pieces = store.lib?.shape.pieces ?? [];

  const identify = (n: number) => {
    setError("");
    api.identifySegment(n).then(
      () => setLit(n - 1),
      (e: unknown) => {
        setLit(null);
        setError(String(e));
      },
    );
  };
  useEffect(() => {
    clearTimeout(timer.current);
    if (selected !== null)
      timer.current = setTimeout(() => identify(selected + 1), IDENTIFY_DEBOUNCE_MS);
    return () => clearTimeout(timer.current);
  }, [selected]);

  const setShape = (next: Piece[]) => store.edit((l) => ({ ...l, shape: { pieces: next } }));
  const change = (n: number) => {
    setConfirm(null);
    setError("");
    // An edit still in its 400 ms debounce must reach Rust first, or the resize would drop it.
    store
      .flushNow()
      .then(() => api.setWiredSegments(n))
      .then(
        (l) => {
          store.reset(l); // Settings are not part of undo: the history starts over.
          setWired(n);
          setSelected(null);
          setLit(null);
          onWired();
        },
        (e: unknown) => setError(String(e)),
      );
  };

  return (
    <div class="space-y-3">
      <SegmentsCard
        strip={strip}
        wired={wired}
        confirm={confirm}
        onStep={(n) => (n < wired ? setConfirm(n) : change(n))}
        onConfirm={(yes) => (yes && confirm !== null ? change(confirm) : setConfirm(null))}
        onNumber={(n) => {
          clearTimeout(timer.current);
          identify(n);
        }}
        onRepair={onRepair}
      />
      <div class="flex items-center gap-2">
        <span class="text-[10px] tracking-wider text-dim uppercase">Start from</span>
      </div>
      <div class="flex flex-wrap gap-2">
        {PRESETS.map((x) => (
          <button
            key={x.name}
            type="button"
            onClick={() => {
              setShape(preset(x.name, pieces.length || wired));
              setSelected(null);
            }}
            class="flex w-21 flex-col items-center gap-1 rounded-lg border border-border bg-card px-2 py-1.5 text-[11px] text-dim hover:border-accent"
          >
            <svg
              width="44"
              height="24"
              viewBox="0 0 44 24"
              fill="none"
              stroke="currentColor"
              stroke-width="3"
              stroke-linecap="round"
              aria-hidden="true"
              class="text-text"
            >
              <path d={x.icon} />
            </svg>
            {x.label}
          </button>
        ))}
      </div>
      <div class="relative">
        <ShapeCanvas
          pieces={pieces}
          selected={selected}
          lit={lit}
          onSelect={setSelected}
          onCommit={setShape}
        />
        <Guide helpOn={helpOn} />
      </div>
      {error && (
        <p role="alert" class="text-red">
          {error}
        </p>
      )}
      <p class="text-xs text-dim">
        Arrows move the selected piece · ⇧ + drag moves the whole shape · ⌘Z undo
      </p>
    </div>
  );
}
