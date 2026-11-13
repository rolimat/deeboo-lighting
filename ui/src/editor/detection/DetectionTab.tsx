import { useEffect, useRef, useState } from "preact/hooks";
import type { Settings, Status } from "../../api";
import { patchSettings } from "../state/settings";
import { api, type DetectorEvent, type Thresholds } from "../api";
import { help } from "../shell/Help";
import { H } from "../help-texts";
import { ChangesSlider } from "./ChangesSlider";
import { pushBeat } from "./history";
import { Timeline } from "./Timeline";

/** The Rust defaults (`Settings::default`). */
const DEFAULTS = { sensitivity: 5, calmKickDb: 16, calmHighDb: 12, riserDb: 7 };
const SEND_DELAY_MS = 300;
const LEVEL_SEGMENTS = 12;

export function Level({ db }: { db: number }) {
  const lit = Math.round(Math.min(1, Math.max(0, (db + 60) / 60)) * LEVEL_SEGMENTS);
  return (
    <span
      role="meter"
      aria-label="Audio level"
      aria-valuemin={-60}
      aria-valuemax={0}
      aria-valuenow={Math.round(db)}
      class="inline-flex gap-0.5"
    >
      {Array.from({ length: LEVEL_SEGMENTS }, (_, i) => (
        <i key={i} class={`block h-3 w-[5px] rounded-sm ${i < lit ? "bg-accent" : "bg-off"}`} />
      ))}
    </span>
  );
}

function StatusBar({ s }: { s: Status | null }) {
  const clock = !!s && s.playing && s.bpm !== null;
  return (
    <div
      data-status-bar
      class="mb-3 flex flex-wrap items-center gap-x-3 gap-y-1 rounded-card border border-border bg-card px-3 py-2 text-small"
    >
      <span
        data-clock
        class={`inline-flex items-center gap-1.5 ${clock ? "text-accent" : "text-dim"}`}
      >
        {clock ? "● MIDI clock playing" : "○ No MIDI clock"}
        <span class="text-dim">
          {clock
            ? `from ${s.midi.source ?? "the virtual port"}`
            : "Start playback in your clock source."}
        </span>
      </span>
      <span class="h-3.5 w-px bg-border" />
      <span
        data-audio
        class={`inline-flex items-center gap-1.5 ${s?.audio.ok ? "text-accent" : "text-dim"}`}
      >
        {s?.audio.ok ? "● Listening" : "○ No audio"}
        <span class="text-dim">
          {s?.audio.ok ? (s.audio.device ?? "system default") : "Pick an input in Setup."}
        </span>
        {s?.audio.ok && s.audio.levelDb !== undefined && <Level db={s.audio.levelDb} />}
      </span>
      <span class="ml-auto text-dim">
        Live view: it moves in real time while the clock plays and the audio input hears the music.
      </span>
    </div>
  );
}

function Empty({ s, onGoSetup }: { s: Status | null; onGoSetup: () => void }) {
  const waiting = !s || !s.playing || s.bpm === null;
  return (
    <div class="flex h-40 flex-col items-center justify-center gap-1.5 rounded-card border border-border bg-bg px-6 text-center text-dim">
      {waiting ? (
        <>
          <b class="text-text">Waiting for MIDI clock</b>
          <span>
            Start playback in your clock source (DJ software, drum machine…). Detection follows the
            beats it sends.
          </span>
          <span>
            Clock input: <span class="text-text">{s?.midi.source ?? "virtual port"}</span> · Audio:{" "}
            {s?.audio.device ?? "system default"} ·{" "}
            <span class={s?.audio.ok ? "text-accent" : ""}>
              {s?.audio.ok ? "level OK" : "no audio"}
            </span>{" "}
            ·{" "}
            <button
              type="button"
              class="text-accent underline"
              onClick={onGoSetup}
              {...help(H.changeInSetup)}
            >
              Change in Setup
            </button>
          </span>
        </>
      ) : (
        <b class="text-text">Moment forced — switch to Auto to see detection</b>
      )}
    </div>
  );
}

export function DetectionTab({ onGoSetup }: { onGoSetup: () => void }) {
  const [s, setS] = useState<Settings | null>(null);
  const [st, setSt] = useState<Status | null>(null);
  const [error, setError] = useState("");
  const [, redraw] = useState(0);
  const ring = useRef<DetectorEvent[]>([]);
  const timer = useRef<number | undefined>(undefined);
  /** Thresholds not yet sent: flushed on unmount so leaving the tab does not drop them. */
  const pending = useRef<Thresholds | null>(null);

  useEffect(() => {
    let alive = true;
    const offs: (() => void)[] = [];
    const keep = (u: () => void) => (alive ? offs.push(u) : u());
    const paint = () => {
      if (!document.hidden) redraw((n) => n + 1);
    };
    api.getSettings().then(
      (v) => alive && setS(v),
      () => setError("Settings unavailable."),
    );
    // Listen first, then read, so no change is missed; a push is newer than the read.
    let pushed = false;
    api.onStatus((v) => ((pushed = true), setSt(v))).then(keep, () => {});
    api.getFullStatus().then(
      (v) => alive && !pushed && setSt(v),
      () => {},
    );
    api
      .onDetector((e) => {
        ring.current = pushBeat(ring.current, e);
        paint();
      })
      .then(keep, () => {});
    api.detectionWatch(true).catch(() => setError("Detection unavailable."));
    document.addEventListener("visibilitychange", paint);
    return () => {
      alive = false;
      offs.forEach((u) => u());
      document.removeEventListener("visibilitychange", paint);
      clearTimeout(timer.current);
      if (pending.current) void api.setDetectionThresholds(pending.current).catch(() => {});
      void api.detectionWatch(false).catch(() => {});
    };
  }, []);

  if (!s) return <p class="text-dim">{error || "Loading…"}</p>;
  const t: Thresholds = { calmKickDb: s.calmKickDb, calmHighDb: s.calmHighDb, riserDb: s.riserDb };
  const setThreshold = (key: keyof Thresholds, value: number) => {
    const next = { ...t, [key]: value };
    setS({ ...s, ...next });
    pending.current = next;
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      pending.current = null;
      api.setDetectionThresholds(next).then(
        () => setError(""),
        (e) => setError(String(e)),
      );
    }, SEND_DELAY_MS);
  };
  const save = (patch: Partial<Settings>) => {
    clearTimeout(timer.current);
    pending.current = null;
    setS({ ...s, ...patch });
    patchSettings(patch).then(
      (v) => (setS(v), setError("")),
      (e) => setError(String(e)),
    );
  };
  const live = !!st && st.playing && st.bpm !== null && st.auto;
  return (
    <div class="max-w-5xl space-y-3">
      <StatusBar s={st} />
      <div class="flex items-start gap-4">
        <span class="t-label w-36 pt-0.5">Moment changes</span>
        <ChangesSlider
          sensitivity={s.sensitivity}
          onChange={(sensitivity) => save({ sensitivity })}
        />
        <button
          type="button"
          onClick={() => save(DEFAULTS)}
          {...help(H.resetDefaults)}
          class="ml-auto rounded-card border border-border bg-card px-3 py-1 text-small"
        >
          Reset to defaults
        </button>
      </div>
      {live ? (
        <Timeline ring={ring.current} t={t} onChange={setThreshold} />
      ) : (
        <Empty s={st} onGoSetup={onGoSetup} />
      )}
      {live && (
        <p class="text-small text-dim">
          The drop is detected when the kick comes back after a Build-up. Drag the dashed lines to
          change when each moment starts. Changes apply on the next beat.
        </p>
      )}
      {error && <p class="text-red">{error}</p>}
    </div>
  );
}
