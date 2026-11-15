import { useRef, useState } from "preact/hooks";
import { H } from "../help-texts";
import { help } from "../shell/Help";
import type { MomentName } from "../../api";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import type { DetectorEvent, Thresholds } from "../api";
import { RING_BEATS } from "./history";
import { Knob } from "./Knob";
import { ThresholdLine } from "./ThresholdLine";
import { ZOOMS, fitLevel, fmtDb, ticks, view, yPct } from "./zoom";

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
  /** Factory default, for the knob's double-click. */
  def: number;
  label: string;
  hint: string;
}
interface LaneDef {
  id: string;
  title: string;
  desc: string;
  band: Band;
  lines: Line[];
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
        def: 16,
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
    lines: [
      {
        key: "riserDb",
        name: "Riser",
        above: true,
        min: 3,
        max: 15,
        def: 7,
        label: "Build-up",
        hint: H.thresholdRiser,
      },
      {
        key: "calmHighDb",
        name: "Highs drop",
        above: false,
        min: 6,
        max: 24,
        def: 12,
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
  const [level, setLevel] = useState(0);
  const refKey = REF[def.band];
  const last = ring[ring.length - 1];
  const ref = last ? Number(last[refKey]) : -30;
  const v = view(ref, level);
  const span = v[0] - v[1];
  const shown = (l: Line) => t[l.key] - (l.above ? 0 : props.shift);
  const lineDb = (l: Line) => ref + (l.above ? shown(l) : -shown(l));
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
            setLevel(fitLevel(ref, [...ring.map((e) => e[def.band]), ...def.lines.map(lineDb)]))
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
            class="pointer-events-none absolute inset-x-0 border-t border-border/40 pl-1 text-small leading-none text-dim"
          >
            {tk}
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
              topPct={yPct(lineDb(l), v)}
              toValue={(clientY) => {
                const r = box.current?.getBoundingClientRect();
                if (!r || !r.height) return val;
                const db = v[0] - ((clientY - r.top) / r.height) * span;
                return l.above ? db - ref : ref - db;
              }}
              onChange={(n) => props.onChange(l.key, n + sh)}
              onOutside={() => {
                let n = level - 1;
                while (
                  n > 0 &&
                  (yPct(lineDb(l), view(ref, n)) < 0 || yPct(lineDb(l), view(ref, n)) > 100)
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
              def={l.def - sh}
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
function runs(ring: DetectorEvent[]): [MomentName, number][] {
  const out: [MomentName, number][] = [];
  for (const e of ring) {
    const tail = out[out.length - 1];
    if (tail?.[0] === e.moment) tail[1] += 1;
    else out.push([e.moment, 1]);
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
        {r.map(([m, n], i) => {
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
  );
}
