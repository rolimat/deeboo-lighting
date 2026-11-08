import { useEffect, useRef, useState } from "preact/hooks";
import { api, type Piece } from "../api";
import { GRID, bounds, rotate, snap } from "./shape";

const MIN_VIEW = { w: 14, h: 8 };
const MARGIN = 2;

/** The row of single cells a strip starts with (mirrors `Shape::row`). */
const rowOf = (n: number): Piece[] =>
  Array.from({ length: n }, (_, i) => ({ x: i * 2, y: 0, angle: 0, len: 1 }));

function fit(pieces: Piece[]) {
  const b = bounds(pieces);
  const w = Math.max(MIN_VIEW.w, b.x1 - b.x0 + 2 * MARGIN);
  const h = Math.max(MIN_VIEW.h, b.y1 - b.y0 + 2 * MARGIN);
  return { x: b.x0 - (w - (b.x1 - b.x0)) / 2, y: b.y0 - (h - (b.y1 - b.y0)) / 2, w, h };
}

const MOVES: Record<string, [number, number]> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
};

/** Lets the user draw how the strip is laid out; click a piece to light that segment. */
export function ShapeEditor({ segments }: { segments: number }) {
  const [pieces, setPieces] = useState<Piece[]>([]);
  const [view, setView] = useState(fit([]));
  const [selected, setSelected] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const svg = useRef<SVGSVGElement>(null);
  const drag = useRef<{ i: number; dx: number; dy: number; moved: boolean } | null>(null);
  const dragged = useRef(false);

  useEffect(() => {
    let gone = false;
    api.getLibrary().then(
      (lib) => {
        if (gone) return;
        const p = lib.shape.pieces.length === segments ? lib.shape.pieces : rowOf(segments);
        setPieces(p);
        setView(fit(p));
      },
      (e: unknown) => !gone && setError(String(e)),
    );
    return () => {
      gone = true;
    };
  }, [segments]);

  const edit = (i: number, f: (p: Piece) => Piece, refit = true) => {
    setNote("");
    const next = pieces.map((p, j) => (j === i ? f(p) : p));
    setPieces(next);
    if (refit) setView(fit(next));
  };
  const identify = (n: number) => {
    setError("");
    api.identifySegment(n).catch((e: unknown) => setError(String(e)));
  };
  const grid = (e: PointerEvent) => {
    const r = svg.current?.getBoundingClientRect();
    if (!r || r.width === 0 || r.height === 0) return { x: 0, y: 0 };
    return {
      x: view.x + ((e.clientX - r.left) / r.width) * view.w,
      y: view.y + ((e.clientY - r.top) / r.height) * view.h,
    };
  };
  const save = async () => {
    setError("");
    setNote("");
    try {
      const lib = await api.getLibrary();
      await api.saveLibrary({ ...lib, shape: { pieces } });
      setNote("Saved.");
    } catch (e) {
      setError(String(e));
    }
  };
  const sel = selected === null ? null : pieces[selected];
  const btn = "rounded-card border border-border px-3 py-1 disabled:opacity-40";

  return (
    <div class="space-y-3">
      <p class="text-dim">
        Drag the pieces to match how the strip is laid out. Click one to light it.
      </p>
      <svg
        ref={svg}
        viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`}
        class="h-72 w-full rounded-card bg-card"
        role="group"
        aria-label="Strip shape"
        onPointerMove={(e) => {
          const d = drag.current;
          if (!d) return;
          const g = grid(e);
          d.moved = true;
          edit(d.i, (p) => ({ ...p, x: snap(g.x - d.dx), y: snap(g.y - d.dy) }), false);
        }}
        onPointerUp={() => {
          if (drag.current?.moved) {
            dragged.current = true;
            setView(fit(pieces));
          }
          drag.current = null;
        }}
      >
        {pieces.map((p, i) => (
          <g
            key={i}
            data-piece
            tabindex={0}
            role="button"
            aria-label={`Segment ${i + 1}`}
            onFocus={() => setSelected(i)}
            onClick={() => {
              setSelected(i);
              // A drag ends in a click on the same piece: that is not an identify.
              if (dragged.current) dragged.current = false;
              else identify(i + 1);
            }}
            onPointerDown={(e) => {
              setSelected(i);
              const g = grid(e);
              drag.current = { i, dx: g.x - p.x, dy: g.y - p.y, moved: false };
              dragged.current = false;
              (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
            }}
            onKeyDown={(e) => {
              const m = MOVES[e.key];
              if (m) {
                e.preventDefault();
                edit(i, (q) => ({ ...q, x: snap(q.x + m[0]), y: snap(q.y + m[1]) }));
              } else if (e.key === "r" || e.key === "R") {
                edit(i, (q) => ({ ...q, angle: rotate(q.angle) }));
              } else if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                identify(i + 1);
              }
            }}
            class={`cursor-grab outline-none ${selected === i ? "text-green" : "text-dim"}`}
          >
            <rect
              x={p.x}
              y={p.y}
              width={p.len}
              height={1}
              rx={0.15}
              transform={`rotate(${p.angle} ${p.x} ${p.y})`}
              fill="currentColor"
              stroke="var(--color-text)"
              stroke-width={0.06}
            />
            <text
              x={p.x}
              y={p.y}
              font-size={0.6}
              fill="var(--color-bg)"
              text-anchor="middle"
              dominant-baseline="central"
              pointer-events="none"
              transform={`rotate(${p.angle} ${p.x} ${p.y}) translate(${p.len / 2} 0.5)`}
            >
              {i + 1}
            </text>
          </g>
        ))}
      </svg>
      <div class="flex flex-wrap gap-2">
        <button
          type="button"
          class={btn}
          disabled={selected === null}
          onClick={() =>
            selected !== null && edit(selected, (p) => ({ ...p, angle: rotate(p.angle) }))
          }
        >
          Rotate
        </button>
        <button
          type="button"
          class={btn}
          disabled={!sel || sel.len <= 1}
          onClick={() =>
            selected !== null &&
            edit(selected, (p) => ({ ...p, len: Math.max(1, p.len - 1) as Piece["len"] }))
          }
        >
          Shorter
        </button>
        <button
          type="button"
          class={btn}
          disabled={!sel || sel.len >= 4}
          onClick={() =>
            selected !== null &&
            edit(selected, (p) => ({ ...p, len: Math.min(4, p.len + 1) as Piece["len"] }))
          }
        >
          Longer
        </button>
        <button type="button" class="rounded-card bg-green px-4 py-1" onClick={() => void save()}>
          Save
        </button>
      </div>
      <p class="sr-only">{`Grid is ${GRID} by ${GRID} cells.`}</p>
      {error && (
        <p role="alert" class="text-red">
          {error}
        </p>
      )}
      {note && <p role="status">{note}</p>}
    </div>
  );
}
