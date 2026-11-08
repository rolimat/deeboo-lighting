import { useEffect, useRef, useState } from "preact/hooks";
import { api, type DetectorEvent, type Thresholds } from "../api";

const DEFAULTS: Thresholds = { calmKickDb: 16, calmHighDb: 12, riserDb: 7 };
const SLIDERS: { key: keyof Thresholds; label: string; min: number; max: number }[] = [
  { key: "calmKickDb", label: "Kick drop", min: 8, max: 30 },
  { key: "calmHighDb", label: "Highs drop", min: 6, max: 24 },
  { key: "riserDb", label: "Riser", min: 3, max: 15 },
];
const MOMENTS = { rhythm: "Rhythm", calm: "Calm", buildUp: "Build-up" } as const;
const SEND_DELAY_MS = 300;
/** Meter scale: -60 dB (empty) to 0 dB (full). */
const pct = (db: number) => Math.round(Math.min(1, Math.max(0, (db + 60) / 60)) * 100);

function Meter(props: { label: string; db: number; refDb: number }) {
  return (
    <div class="space-y-1">
      <div class="text-dim">{props.label}</div>
      <div
        role="meter"
        aria-label={props.label}
        aria-valuemin={-60}
        aria-valuemax={0}
        aria-valuenow={Math.round(props.db)}
        class="relative h-3 rounded-card bg-card"
      >
        <div class="h-full rounded-card bg-green" style={{ width: `${pct(props.db)}%` }} />
        <div class="absolute top-0 h-full w-0.5 bg-text" style={{ left: `${pct(props.refDb)}%` }} />
      </div>
    </div>
  );
}

export function DetectionPanel() {
  const [t, setT] = useState<Thresholds | null>(null);
  const [beat, setBeat] = useState<DetectorEvent | null>(null);
  const [error, setError] = useState("");
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    api.getSettings().then(
      (s) =>
        !gone && setT({ calmKickDb: s.calmKickDb, calmHighDb: s.calmHighDb, riserDb: s.riserDb }),
      () => setError("Settings unavailable."),
    );
    api.onDetector(setBeat).then((u) => (gone ? u() : (off = u)));
    api.detectionWatch(true).catch(() => setError("Meters unavailable."));
    return () => {
      gone = true;
      off?.();
      clearTimeout(timer.current);
      void api.detectionWatch(false).catch(() => {});
    };
  }, []);

  const send = (next: Thresholds, delay: number) => {
    setT(next);
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      api.setDetectionThresholds(next).then(
        () => setError(""),
        (e) => setError(String(e)),
      );
    }, delay);
  };

  if (!t) return <p class="text-dim">{error || "Loading…"}</p>;
  return (
    <div class="max-w-md space-y-6">
      <div class="space-y-3">
        <Meter label="Kick" db={beat?.kickDb ?? -60} refDb={beat?.kickRefDb ?? -60} />
        <Meter label="Sub" db={beat?.subDb ?? -60} refDb={beat?.subRefDb ?? -60} />
        <Meter label="Highs" db={beat?.highDb ?? -60} refDb={beat?.highRefDb ?? -60} />
        <p>
          <span class="rounded-card bg-card px-3 py-1">
            {beat ? MOMENTS[beat.moment] : "No audio"}
          </span>
        </p>
      </div>
      <div class="space-y-3">
        {SLIDERS.map(({ key, label, min, max }) => (
          <label key={key} class="flex items-center gap-3">
            <span class="w-24">{label}</span>
            <input
              type="range"
              aria-label={label}
              min={min}
              max={max}
              step={1}
              value={t[key]}
              onInput={(e) => send({ ...t, [key]: Number(e.currentTarget.value) }, SEND_DELAY_MS)}
              class="flex-1"
            />
            <span class="w-14 text-right text-dim">{t[key]} dB</span>
          </label>
        ))}
        <button
          type="button"
          onClick={() => send(DEFAULTS, 0)}
          class="rounded-card border border-border px-4 py-2"
        >
          Reset
        </button>
        {error && <p class="text-red">{error}</p>}
      </div>
    </div>
  );
}
