import { useRef, useState } from "preact/hooks";
import { H } from "../help-texts";
import { help } from "../shell/Help";
import type { MomentName } from "../../api";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import type { Cause, DetectorEvent, Thresholds } from "../api";
import { CAUSE_WORDS } from "./causes";
import { RING_BEATS } from "./history";
import { DEBUG } from "./debug";
import { DebugPanel } from "./DebugPanel";
import { DEFAULTS } from "./defaults";
import { Knob } from "./Knob";
import { ThresholdLine } from "./ThresholdLine";
import { ZOOMS, centreOf, fitLevel, fmtDb, ticks, view, yPct } from "./zoom";

const MOMENTS: Record<MomentName, [IconName, string]> = {
  rhythm: ["rhythm", "Rhythm"],
  calm: ["calm", "Calm"],
  buildUp: ["buildup", "Build-up"],
};
const COL = "grid grid-cols-[120px_44px_1fr_7.5rem]";
const ZBTN =
  "h-5 w-8 rounded border border-border text-small leading-none enabled:hover:border-accent disabled:opacity-40";
const HEAD = "border-b border-r border-border bg-card p-2 text-small";
const BAR_LABELS = ["8 bars ago", "7", "6", "5", "4", "3", "2", "1"];

type Band = "kickDb" | "subDb" | "highDb";
interface Line {
  key: keyof Thresholds;
  name: string;
  /** Calm lines sit below the reference, Build-up above it. */
  above: boolean;
  min: number;
  max: number;
  label: string;
  hint: string;
}
interface LaneDef {
  id: string;
  title: string;
  desc: string;
  band: Band;
  lines: Line[];
  /** Zoom level the lane opens at. */
  zoom?: number;
}
const LANES: LaneDef[] = [
  {
    id: "kick",
    title: "Kick",
    desc: "Bass drum. Falling far below its reference means a break → Calm.",
    band: "kickDb",
    lines: [
      {
        key: "calmKickDb",
        name: "Kick drop",
        above: false,
        min: 8,
        max: 30,
        label: "Calm",
        hint: H.thresholdKick,
      },
    ],
  },
  { id: "sub", title: "Sub", desc: "Low bass. Confirms the break.", band: "subDb", lines: [] },
  {
    id: "high",
    title: "Highs",
    desc: "Hi-hats and noise. A steady climb means a riser → Build-up; a fall helps Calm.",
    band: "highDb",
    zoom: 2,
    lines: [
      {
        key: "riserDb",
        name: "Riser",
        above: true,
        min: 1,
        max: 15,
        label: "Build-up",
        hint: H.thresholdRiser,
      },
      {
        key: "calmHighDb",
        name: "Highs drop",
        above: false,
        min: 6,
        max: 24,
        label: "Calm",
        hint: H.thresholdHigh,
      },
    ],
  },
];
const REF: Record<Band, keyof DetectorEvent> = {
  kickDb: "kickRefDb",
  subDb: "subRefDb",
  highDb: "highRefDb",
};

const points = (ring: DetectorEvent[], v: [number, number], pick: (e: DetectorEvent) => number) =>
  ring
    .map(
      (e, i) =>
        `${(((RING_BEATS - ring.length + i) / (RING_BEATS - 1)) * 1000).toFixed(1)},${yPct(pick(e), v).toFixed(1)}`,
    )
    .join(" ");

/** The bar means the detector decided on, held between bars as a stepped line. */
function barSteps(ring: DetectorEvent[], v: [number, number]): string {
  const out: string[] = [];
  let y: number | null = null;
  ring.forEach((e, i) => {
    const x = (((RING_BEATS - ring.length + i) / (RING_BEATS - 1)) * 1000).toFixed(1);
    const next = e.progress.highBarDb;
    if (next === null && y === null) return;
    if (next !== null) {
      if (y !== null) out.push(`${x},${y.toFixed(1)}`);
      y = yPct(next, v);
    }
    out.push(`${x},${(y as number).toFixed(1)}`);
  });
  return out.join(" ");
}

function Lane(props: {
  def: LaneDef;
  ring: DetectorEvent[];
  t: Thresholds;
  /** dB the detector lowers the calm thresholds by (sensitivity - 5); lines show what it applies. */
  shift: number;
  onChange: (key: keyof Thresholds, value: number) => void;
}) {
  const { def, ring, t } = props;
  const box = useRef<HTMLDivElement>(null);
  const refKey = REF[def.band];
  const last = ring[ring.length - 1];
  const liveRef = last ? Number(last[refKey]) : -30;
  const shown = (l: Line) => t[l.key] - (l.above ? 0 : props.shift);
  // The view centre and reference are frozen while a line is dragged, so the scale does not slide under the pointer.
  const held = useRef<[number, number] | null>(null);
  const [, redraw] = useState(0);
  const ref = held.current?.[1] ?? liveRef;
  const lineDb = (l: Line) => ref + (l.above ? shown(l) : -shown(l));
  const centre = held.current?.[0] ?? centreOf(ref, def.lines.map(lineDb));
  // Opens at the closest zoom (up to `def.zoom`) that holds the reference and every line; decided once.
  const [level, setLevel] = useState(() =>
    Math.min(def.zoom ?? 0, fitLevel(centre, [ref, ...def.lines.map(lineDb)])),
  );
  const v = view(centre, level);
  const span = v[0] - v[1];
  return (
    <>
      <div class={HEAD}>
        <b class="t-head">{def.title}</b>
        <small class="mt-0.5 block text-dim">{def.desc}</small>
      </div>
      <div
        data-lane-row={def.id}
        class="flex flex-col items-center justify-center gap-1 border-b border-r border-border bg-card"
      >
        <button
          type="button"
          aria-label="Zoom in"
          disabled={level >= ZOOMS.length - 1}
          onClick={() => setLevel(level + 1)}
          class={ZBTN}
          {...help(H.bandZoomIn)}
        >
          +
        </button>
        <span class="text-small text-dim">×{ZOOMS[level]}</span>
        <button
          type="button"
          aria-label="Zoom out"
          disabled={level <= 0}
          onClick={() => setLevel(level - 1)}
          class={ZBTN}
          {...help(H.bandZoomOut)}
        >
          −
        </button>
        <button
          type="button"
          aria-label="Zoom to fit"
          onClick={() =>
            setLevel(fitLevel(centre, [...ring.map((e) => e[def.band]), ...def.lines.map(lineDb)]))
          }
          class={ZBTN}
          {...help(H.bandZoomFit)}
        >
          Fit
        </button>
      </div>
      <div
        ref={box}
        data-lane={def.id}
        class="relative h-24 overflow-hidden border-b border-border bg-bg"
      >
        {ticks(v).map((tk) => (
          <div
            key={tk}
            data-scale
            style={{ top: `${yPct(tk, v)}%` }}
            class="pointer-events-none absolute inset-x-0 border-t border-border/40"
          >
            <span
              data-scale-num
              class="absolute left-1 top-0 z-[15] -translate-y-1/2 rounded bg-bg px-0.5 text-small leading-none text-dim"
            >
              {tk}
            </span>
          </div>
        ))}
        <svg
          viewBox="0 0 1000 100"
          preserveAspectRatio="none"
          class="absolute inset-0 h-full w-full"
        >
          <polyline
            points={points(ring, v, (e) => Number(e[refKey]))}
            fill="none"
            stroke="currentColor"
            stroke-width="1"
            vector-effect="non-scaling-stroke"
            class="text-dim"
          />
          {DEBUG && def.band === "highDb" && (
            <polyline
              data-debug
              data-bar-steps
              points={barSteps(ring, v)}
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              vector-effect="non-scaling-stroke"
              class="text-accent opacity-40"
            />
          )}
          <polyline
            points={points(ring, v, (e) => e[def.band])}
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            vector-effect="non-scaling-stroke"
            class="text-text"
          />
        </svg>
        {def.lines.map((l) => {
          const sh = l.above ? 0 : props.shift;
          const val = shown(l);
          return (
            <ThresholdLine
              key={l.key}
              name={l.name}
              hint={l.hint}
              words={`${l.label} when ${fmtDb(val)} dB ${l.above ? "above" : "below"}`}
              value={val}
              min={l.min - sh}
              max={l.max - sh}
              // At x1 the view is the whole scale: keep a line below the floor visible at the edge.
              topPct={
                level === 0 ? Math.min(100, Math.max(0, yPct(lineDb(l), v))) : yPct(lineDb(l), v)
              }
              toValue={(clientY) => {
                const r = box.current?.getBoundingClientRect();
                if (!r || !r.height) return val;
                const db = v[0] - ((clientY - r.top) / r.height) * span;
                return l.above ? db - ref : ref - db;
              }}
              onChange={(n) => props.onChange(l.key, n + sh)}
              onDrag={(on) => {
                held.current = on ? [centre, ref] : null;
                redraw((n) => n + 1);
              }}
              onOutside={() => {
                let n = level - 1;
                while (
                  n > 0 &&
                  (yPct(lineDb(l), view(centre, n)) < 0 || yPct(lineDb(l), view(centre, n)) > 100)
                )
                  n--;
                setLevel(Math.max(0, n));
              }}
            />
          );
        })}
      </div>
      <div
        data-knobs={def.id}
        class="flex items-center justify-center gap-1 border-b border-l border-border bg-card"
      >
        {def.lines.map((l) => {
          const sh = l.above ? 0 : props.shift;
          return (
            <Knob
              key={l.key}
              name={`${l.name} knob`}
              label={l.label}
              above={l.above}
              value={shown(l)}
              min={l.min - sh}
              max={l.max - sh}
              def={DEFAULTS[l.key] - sh}
              hint={l.hint}
              onChange={(n) => props.onChange(l.key, n + sh)}
            />
          );
        })}
      </div>
    </>
  );
}

/** What the show used per beat, as runs of equal moments (oldest first). */
function runs(ring: DetectorEvent[]): [MomentName, number, Cause | null][] {
  const out: [MomentName, number, Cause | null][] = [];
  for (const e of ring) {
    const tail = out[out.length - 1];
    if (tail?.[0] === e.moment) tail[1] += 1;
    else out.push([e.moment, 1, e.cause]);
  }
  return out;
}

/** Arrangement-style view of the last 32 beats; redrawn by its parent on each new beat. */
export function Timeline(props: {
  ring: DetectorEvent[];
  t: Thresholds;
  shift: number;
  onChange: (key: keyof Thresholds, value: number) => void;
}) {
  const r = runs(props.ring);
  return (
    <>
      <div class={`${COL} overflow-hidden rounded-card border border-border`}>
        <div class={`${HEAD} text-dim`}>Last 8 bars</div>
        <div class="border-b border-r border-border bg-card p-2 text-small text-dim">Zoom</div>
        <div class="grid grid-cols-8 border-b border-border bg-card text-small text-dim">
          {BAR_LABELS.map((b, i) => (
            <span key={b} data-bar class="flex justify-between border-l border-border px-1 py-1">
              {b}
              {i === 7 && <b class="text-accent">now ▸</b>}
            </span>
          ))}
        </div>
        <div class="border-b border-l border-border bg-card" />
        <div class={HEAD}>
          <b class="t-head">Moment</b>
          <small class="mt-0.5 block text-dim">What the show used</small>
        </div>
        <div class="border-b border-r border-border bg-card" />
        <div class="flex h-[34px] border-b border-border bg-bg">
          <div style={{ flex: RING_BEATS - props.ring.length }} />
          {r.map(([m, n, cause], i) => {
            const [icon, name] = MOMENTS[m];
            const now = i === r.length - 1;
            return (
              <div
                key={i}
                data-moment-run
                {...(now ? { "data-now": "" } : {})}
                style={{ flex: n }}
                class={`flex min-w-0 items-center gap-1 overflow-hidden whitespace-nowrap border-r border-bg bg-card px-1.5 text-small ${now ? "outline outline-1 -outline-offset-1 outline-accent" : ""}`}
              >
                <Icon name={icon} />
                {name}
                {DEBUG && cause && (
                  <span data-debug class="text-dim">
                    · {CAUSE_WORDS[cause]}
                  </span>
                )}
              </div>
            );
          })}
        </div>
        <div class="border-b border-l border-border bg-card" />
        {LANES.map((def) => (
          <Lane
            key={def.id}
            def={def}
            ring={props.ring}
            t={props.t}
            shift={props.shift}
            onChange={props.onChange}
          />
        ))}
      </div>
      {DEBUG && <DebugPanel last={props.ring[props.ring.length - 1]} riserDb={props.t.riserDb} />}
    </>
  );
}
