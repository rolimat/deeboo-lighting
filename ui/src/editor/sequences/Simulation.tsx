import { useEffect, useState } from "preact/hooks";
import type { Palette, Piece, Sequence } from "../api";
import { bounds } from "../strip/shape";
import { stepAt } from "./player";
import { roleColour } from "./RolePicker";

const MIN_BPM = 60;
const MAX_BPM = 180;
const MARGIN = 1;

/** Plays a sequence on the strip's shape at a chosen tempo, with no clock or strip involved. */
export function Simulation({
  sequence,
  pieces,
  palette,
}: {
  sequence: Sequence;
  pieces: Piece[];
  palette: Palette;
}) {
  const [bpm, setBpm] = useState(120);
  const [playing, setPlaying] = useState(false);
  const [visible, setVisible] = useState(document.visibilityState === "visible");
  const [half, setHalf] = useState(0);

  useEffect(() => {
    const onVis = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);

  const running = playing && visible;
  useEffect(() => {
    if (!running) return;
    let id = 0;
    let last: number | null = null;
    let pos = 0;
    const frame = (ts: number) => {
      // Elapsed time times the tempo, so a BPM change takes effect without a jump.
      if (last !== null) pos += ((ts - last) * bpm) / 30_000;
      last = ts;
      setHalf((h) => (Math.floor(pos) === h ? h : Math.floor(pos)));
      id = requestAnimationFrame(frame);
    };
    pos = half;
    id = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(id);
    // `half` is only the starting point of a run.
  }, [running, bpm]);

  const match = sequence.steps.every((s) => s.roles.length === pieces.length);
  const step = sequence.steps[stepAt(sequence, half)];
  const b = bounds(pieces);

  return (
    <div class="space-y-2">
      <div class="flex items-center gap-3">
        <button
          type="button"
          onClick={() => setPlaying((p) => !p)}
          class="rounded-card border border-border px-3 py-1"
        >
          {playing ? "Pause" : "Play"}
        </button>
        <label class="flex items-center gap-2 text-sm text-dim">
          BPM
          <input
            type="number"
            aria-label="BPM"
            min={MIN_BPM}
            max={MAX_BPM}
            value={bpm}
            onInput={(e) => {
              const v = Math.round(Number(e.currentTarget.value));
              const next =
                Number.isFinite(v) && v > 0 ? Math.min(MAX_BPM, Math.max(MIN_BPM, v)) : bpm;
              e.currentTarget.value = String(next);
              setBpm(next);
            }}
            class="w-20 rounded-card border border-border bg-card px-2 py-1 text-text"
          />
        </label>
      </div>
      {match && step ? (
        <svg
          viewBox={`${b.x0 - MARGIN} ${b.y0 - MARGIN} ${b.x1 - b.x0 + 2 * MARGIN} ${b.y1 - b.y0 + 2 * MARGIN}`}
          class="h-40 w-full rounded-card bg-card"
          role="img"
          aria-label="Simulation"
        >
          {pieces.map((p, i) => (
            <rect
              key={i}
              x={p.x}
              y={p.y}
              width={p.len}
              height={1}
              rx={0.15}
              transform={`rotate(${p.angle} ${p.x} ${p.y})`}
              fill={roleColour(step.roles[i] ?? "off", palette)}
              stroke="var(--color-text)"
              stroke-width={0.06}
            />
          ))}
        </svg>
      ) : (
        <p class="text-dim">
          The strip shape has {pieces.length} pieces but this sequence does not match it, so it
          cannot be simulated.
        </p>
      )}
    </div>
  );
}
