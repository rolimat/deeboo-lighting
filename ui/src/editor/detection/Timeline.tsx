import { useRef } from "preact/hooks";
import { H } from "../help-texts";
import type { MomentName } from "../../api";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import type { DetectorEvent, Thresholds } from "../api";
import { RING_BEATS } from "./history";
import { ThresholdLine } from "./ThresholdLine";

const MOMENTS: Record<MomentName, [IconName, string]> = {
  rhythm: ["rhythm", "Rhythm"],
  calm: ["calm", "Calm"],
  buildUp: ["buildup", "Build-up"],
};
/** Lanes span -60 dB (bottom) to 0 dB (top). */
const FLOOR_DB = 60;
const yPct = (db: number) => Math.min(100, Math.max(0, (-db / FLOOR_DB) * 100));
const COL = "grid grid-cols-[120px_1fr_12rem]";
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
    lines: [
      {
        key: "riserDb",
        name: "Riser",
        above: true,
        min: 3,
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

const points = (ring: DetectorEvent[], pick: (e: DetectorEvent) => number) =>
  ring
    .map(
      (e, i) =>
        `${(((RING_BEATS - ring.length + i) / (RING_BEATS - 1)) * 1000).toFixed(1)},${yPct(pick(e)).toFixed(1)}`,
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
  const refKey = REF[def.band];
  const last = ring[ring.length - 1];
  const ref = last ? Number(last[refKey]) : -30;
  return (
    <>
      <div class={HEAD}>
        <b class="t-head">{def.title}</b>
        <small class="mt-0.5 block text-dim">{def.desc}</small>
      </div>
      <div ref={box} data-lane={def.id} class="relative h-20 border-b border-border bg-bg">
        <svg
          viewBox="0 0 1000 100"
          preserveAspectRatio="none"
          class="absolute inset-0 h-full w-full"
        >
          <polyline
            points={points(ring, (e) => Number(e[refKey]))}
            fill="none"
            stroke="currentColor"
            stroke-width="1"
            vector-effect="non-scaling-stroke"
            class="text-dim"
          />
          <polyline
            points={points(ring, (e) => e[def.band])}
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            vector-effect="non-scaling-stroke"
            class="text-text"
          />
        </svg>
        {def.lines.map((l) => {
          const sh = l.above ? 0 : props.shift;
          const v = t[l.key] - sh;
          return (
            <ThresholdLine
              key={l.key}
              name={l.name}
              hint={l.hint}
              words={`${l.label} when ${v} dB ${l.above ? "above" : "below"}`}
              value={v}
              min={l.min - sh}
              max={l.max - sh}
              topPct={yPct(ref + (l.above ? v : -v))}
              toValue={(clientY) => {
                const r = box.current?.getBoundingClientRect();
                if (!r || !r.height) return v;
                const db = -((clientY - r.top) / r.height) * FLOOR_DB;
                return l.above ? db - ref : ref - db;
              }}
              onChange={(n) => props.onChange(l.key, n + sh)}
            />
          );
        })}
      </div>
      <div data-readouts class="relative h-20 border-b border-l border-border bg-card">
        {def.lines.map((l) => {
          const v = t[l.key] - (l.above ? 0 : props.shift);
          return (
            <span
              key={l.key}
              style={{ top: `${yPct(ref + (l.above ? v : -v))}%` }}
              class="absolute left-1.5 -translate-y-1/2 select-none whitespace-nowrap rounded border border-accent bg-card px-1.5 text-small text-accent"
            >
              {l.label} when {v} dB {l.above ? "above" : "below"}
            </span>
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
