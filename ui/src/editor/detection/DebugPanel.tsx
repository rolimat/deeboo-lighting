import type { DetectorEvent } from "../api";

const f = (n: number) => n.toFixed(1);
const sg = (n: number) => `${n >= 0 ? "+" : ""}${f(n)}`;

/** "Next moment": how close the Build-up evidence is, from the newest beat (debug build only). */
export function DebugPanel({ last }: { last: DetectorEvent | undefined }) {
  const p = last?.progress;
  if (!p) return null;
  const rows: [boolean, string][] = [
    [
      p.riserBars >= p.riserNeedBars && p.riserRiseDb >= p.riserNeedDb && p.riserNearRef,
      `Riser ${p.riserBars}/${p.riserNeedBars} bars, ${sg(p.riserRiseDb)} of ${f(p.riserNeedDb)} dB${p.riserNearRef ? "" : ", near ref ✗"}`,
    ],
    [
      p.approachCalmBars >= p.approachNeedBars &&
        p.approachAboveDb >= p.approachNeedAboveDb &&
        p.approachRiseDb >= p.approachNeedRiseDb,
      `Approach ${p.approachCalmBars}/${p.approachNeedBars} bars in Calm, ${sg(p.approachAboveDb)} of ${f(p.approachNeedAboveDb)} dB over ref, rise ${f(p.approachRiseDb)} of ${f(p.approachNeedRiseDb)}`,
    ],
  ];
  return (
    <div data-debug="panel" class="rounded-card border border-border bg-card p-2 text-small">
      <b class="t-head">Next moment</b>
      {rows.map(([ok, text]) => (
        <div key={text} class={ok ? "text-accent" : "text-dim"}>
          {ok ? "✓" : "✗"} {text}
        </div>
      ))}
    </div>
  );
}
