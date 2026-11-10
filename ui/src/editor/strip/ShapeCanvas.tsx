import { useRef, useState } from "preact/hooks";
import type { Piece } from "../api";
import { Icon } from "../../icons/Icon";
import { bounds, endpoints, fits, magnet, normalise, snap, spin, turn } from "./shape";

interface View {
  x: number;
  y: number;
  w: number;
  h: number;
}
const MARGIN = 5;
const MIN_W = 16;

/** The viewBox always has the canvas's 2:1 shape, so pointer maths needs no letterboxing. */
function fit(pieces: Piece[]): View {
  const b = bounds(pieces);
  const w = Math.max(MIN_W, b.x1 - b.x0 + 2 * MARGIN, 2 * (b.y1 - b.y0 + 2 * MARGIN));
  return { x: (b.x0 + b.x1) / 2 - w / 2, y: (b.y0 + b.y1) / 2 - w / 4, w, h: w / 2 };
}

const MOVES: Record<string, [number, number]> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
};

type Drag =
  | { kind: "piece"; i: number; ox: number; oy: number; base: Piece[] }
  | { kind: "shape"; gx: number; gy: number; base: Piece[] }
  | { kind: "rotate"; i: number; base: Piece[] };

const tool =
  "grid h-6 w-6 place-items-center rounded-md border border-border text-dim hover:text-text";

/** The free canvas: drag pieces anywhere (it grows), turn them by the handle, nudge with the arrows. */
export function ShapeCanvas(p: {
  pieces: Piece[];
  selected: number | null;
  /** The piece lit on the real strip. */
  lit: number | null;
  onSelect: (i: number | null) => void;
  /** One call per finished gesture: the new, already normalised pieces. */
  onCommit: (next: Piece[]) => void;
}) {
  const svg = useRef<SVGSVGElement>(null);
  const drag = useRef<Drag | null>(null);
  const [draft, setDraft] = useState<Piece[] | null>(null);
  const [frozen, setFrozen] = useState<View | null>(null);
  const shown = draft ?? p.pieces;
  const view = frozen ?? fit(p.pieces);

  const grid = (e: MouseEvent) => {
    const r = svg.current?.getBoundingClientRect();
    if (!r || r.width === 0 || r.height === 0) return { x: 0, y: 0 };
    return {
      x: view.x + ((e.clientX - r.left) / r.width) * view.w,
      y: view.y + ((e.clientY - r.top) / r.height) * view.h,
    };
  };
  const commit = (next: Piece[]) => {
    const n = normalise(next);
    if (fits(n)) p.onCommit(n);
  };
  const replace = (i: number, q: Piece) => p.pieces.map((o, j) => (j === i ? q : o));
  const start = (e: PointerEvent, d: Drag) => {
    drag.current = d;
    setFrozen(view);
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
  };
  const sel = p.selected === null ? undefined : p.pieces[p.selected];

  const onMove = (e: PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const g = grid(e);
    if (d.kind === "piece") {
      const m = magnet(d.base, d.i, snap(g.x - d.ox), snap(g.y - d.oy));
      const q = d.base[d.i];
      if (q) setDraft(d.base.map((o, j) => (j === d.i ? { ...q, ...m } : o)));
    } else if (d.kind === "shape") {
      const dx = snap(g.x - d.gx);
      const dy = snap(g.y - d.gy);
      setDraft(d.base.map((o) => ({ ...o, x: o.x + dx, y: o.y + dy })));
    } else {
      const q = d.base[d.i];
      if (!q) return;
      const a = endpoints(q).a;
      const deg = (Math.atan2(g.y - a.y, g.x - a.x) * 180) / Math.PI;
      setDraft(d.base.map((o, j) => (j === d.i ? spin(q, turn(Math.round(deg / 15) * 15, 0)) : o)));
    }
  };
  const onUp = () => {
    if (draft) commit(draft);
    drag.current = null;
    setDraft(null);
    setFrozen(null);
  };

  const tip = sel && endpoints(sel).b;
  const dir = sel ? (sel.angle * Math.PI) / 180 : 0;
  const pct = (v: number, o: number, s: number) => Math.min(75, Math.max(0, ((v - o) / s) * 100));

  return (
    <div class="relative">
      <svg
        ref={svg}
        viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`}
        class="aspect-2/1 w-full touch-none rounded-card border border-border bg-bg"
        role="group"
        aria-label="Strip shape"
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={onUp}
      >
        <defs>
          <pattern id="dots" width={1} height={1} patternUnits="userSpaceOnUse">
            <circle cx={0} cy={0} r={0.04} class="fill-border" />
          </pattern>
        </defs>
        <rect x={view.x} y={view.y} width={view.w} height={view.h} fill="url(#dots)" />
        {shown.map((q, i) => {
          const on = p.selected === i;
          const c = {
            x: (endpoints(q).a.x + endpoints(q).b.x) / 2,
            y: (endpoints(q).a.y + endpoints(q).b.y) / 2,
          };
          return (
            <g
              key={i}
              data-piece
              tabindex={0}
              role="button"
              aria-label={`Segment ${i + 1}`}
              class="cursor-grab outline-none"
              onFocus={() => p.onSelect(i)}
              onPointerDown={(e) => {
                p.onSelect(i);
                const g = grid(e);
                start(
                  e,
                  e.shiftKey
                    ? { kind: "shape", gx: g.x, gy: g.y, base: p.pieces }
                    : { kind: "piece", i, ox: g.x - q.x, oy: g.y - q.y, base: p.pieces },
                );
              }}
              onKeyDown={(e) => {
                const m = MOVES[e.key];
                if (!m) return;
                e.preventDefault();
                commit(replace(i, { ...q, x: q.x + m[0], y: q.y + m[1] }));
              }}
            >
              <rect
                transform={`translate(${q.x} ${q.y}) rotate(${q.angle})`}
                x={0.04}
                y={0.25}
                width={q.len - 0.08}
                height={0.5}
                rx={0.25}
                stroke-width={0.07}
                class={`${p.lit === i ? "fill-accent" : on ? "fill-dim" : "fill-border"} ${on ? "stroke-accent" : "stroke-transparent"}`}
              />
              <text
                x={c.x}
                y={c.y}
                font-size={0.34}
                text-anchor="middle"
                dominant-baseline="central"
                pointer-events="none"
                class={p.lit === i ? "fill-bg" : "fill-text"}
              >
                {i + 1}
              </text>
            </g>
          );
        })}
        {sel && tip && p.selected !== null && (
          <circle
            data-handle
            role="button"
            aria-label={`Rotate segment ${p.selected + 1}`}
            cx={tip.x + 0.45 * Math.cos(dir)}
            cy={tip.y + 0.45 * Math.sin(dir)}
            r={0.2}
            stroke-width={0.07}
            class="cursor-grab fill-bg stroke-accent"
            onPointerDown={(e) => {
              e.stopPropagation();
              if (p.selected !== null) start(e, { kind: "rotate", i: p.selected, base: p.pieces });
            }}
          />
        )}
      </svg>
      {sel && p.selected !== null && tip && (
        <div
          class="absolute flex items-center gap-1 rounded-lg border border-border bg-card p-1"
          style={{
            left: `${pct(tip.x, view.x, view.w)}%`,
            top: `${Math.max(0, pct(tip.y, view.y, view.h) - 14)}%`,
          }}
        >
          <button
            type="button"
            class={tool}
            aria-label="Rotate −15°"
            title="Rotate −15°"
            onClick={() => commit(replace(p.selected as number, spin(sel, turn(sel.angle, -15))))}
          >
            <Icon name="rotateccw" />
          </button>
          <button
            type="button"
            class={tool}
            aria-label="Rotate +15°"
            title="Rotate +15°"
            onClick={() => commit(replace(p.selected as number, spin(sel, turn(sel.angle, 15))))}
          >
            <Icon name="rotatecw" />
          </button>
          {p.lit === p.selected && (
            <span class="grid h-6 w-6 place-items-center text-accent" title="Lit on the strip">
              <Icon name="lightbulb" />
            </span>
          )}
          <span class="px-1 text-small text-dim">{`Segment ${p.selected + 1} · ${Math.round(sel.angle)}°`}</span>
        </div>
      )}
    </div>
  );
}
