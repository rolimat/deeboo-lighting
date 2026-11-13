import type { ComponentChildren } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";
import type { Status } from "../../api";
import { api, type Palette, type Piece, type Sequence } from "../api";
import { Icon } from "../../icons/Icon";
import { help } from "../shell/Help";
import { H } from "../help-texts";
import { bounds } from "../strip/shape";
import { stepAt } from "./player";
import { roleColour } from "./RolePicker";

const MARGIN = 1;
const FREE_BPM = 120;

/** Plays a sequence on the strip's shape: on the MIDI clock's beat when it plays, else at 120 BPM. */
export function Simulation({
  sequence,
  pieces,
  palette,
  onHalf,
  children,
}: {
  sequence: Sequence;
  pieces: Piece[];
  palette: Palette;
  /** The playing half-beat (counted from the start), or null while paused. */
  onHalf?: (half: number | null) => void;
  /** Extra controls on the same row, after the shape. */
  children?: ComponentChildren;
}) {
  const [playing, setPlaying] = useState(false);
  const [visible, setVisible] = useState(document.visibilityState === "visible");
  const [half, setHalf] = useState(0);
  const bpm = useRef(FREE_BPM);
  const sync = useRef<number | null>(null); // half-beat in the phrase the clock just reported
  const phrase = useRef(64); // half-beats in a phrase

  useEffect(() => {
    const onVis = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);

  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    const apply = (st: Status) => {
      if (st.playing && st.bpm && st.beatInBar && st.barInPhrase) {
        bpm.current = st.bpm;
        phrase.current = st.phraseBars * 8;
        sync.current = ((st.barInPhrase - 1) * 4 + (st.beatInBar - 1)) * 2;
      } else bpm.current = FREE_BPM;
    };
    api.onStatus(apply).then(
      (u) => (alive ? (off = u) : u()),
      () => {},
    );
    api.getFullStatus().then(
      (st) => alive && apply(st),
      () => {},
    );
    return () => {
      alive = false;
      off?.();
    };
  }, []);

  const running = playing && visible;
  useEffect(() => {
    if (!running) return;
    let id = 0;
    let last: number | null = null;
    let pos = half;
    const frame = (ts: number) => {
      if (last !== null) pos += ((ts - last) * bpm.current) / 30_000;
      if (sync.current !== null) {
        // The clock only knows the phrase position: keep the free-running bar (a sequence can
        // be longer than a phrase) and move to the nearest point with that phase.
        const P = phrase.current;
        let d = (((sync.current - pos) % P) + P) % P;
        if (d > P / 2) d -= P;
        pos = pos + d < 0 ? pos + d + P : pos + d;
        sync.current = null;
      }
      last = ts;
      setHalf((h) => (Math.floor(pos) === h ? h : Math.floor(pos)));
      id = requestAnimationFrame(frame);
    };
    id = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(id);
    // `half` is only the starting point of a run.
  }, [running]);

  useEffect(() => onHalf?.(playing ? half : null), [playing, half]);

  const match = sequence.steps.every((s) => s.roles.length === pieces.length);
  const step = sequence.steps[stepAt(sequence, half)];
  const b = bounds(pieces);

  return (
    <div class="flex items-center gap-3 rounded-card bg-black/40 p-3">
      <button
        type="button"
        aria-label={playing ? "Pause" : "Play"}
        aria-pressed={playing}
        {...help(H.simulation)}
        onClick={() => setPlaying((p) => !p)}
        class={`grid h-7 w-7 shrink-0 place-items-center rounded-lg border ${playing ? "border-accent text-accent" : "border-border text-dim"}`}
      >
        <Icon name={playing ? "pause" : "play"} />
      </button>
      {match && step ? (
        <svg
          viewBox={`${b.x0 - MARGIN} ${b.y0 - MARGIN} ${b.x1 - b.x0 + 2 * MARGIN} ${b.y1 - b.y0 + 2 * MARGIN}`}
          class="h-20 min-w-0 flex-1"
          role="img"
          aria-label="Simulation"
        >
          {pieces.map((p, i) => (
            <rect
              key={i}
              x={p.x}
              y={p.y}
              width={p.len}
              height={0.7}
              rx={0.35}
              transform={`rotate(${p.angle} ${p.x} ${p.y})`}
              fill={roleColour(step.roles[i] ?? "off", palette)}
            />
          ))}
        </svg>
      ) : (
        <p class="text-dim">
          The strip shape has {pieces.length} pieces but this sequence does not match it, so it
          cannot be simulated.
        </p>
      )}
      {children}
    </div>
  );
}
