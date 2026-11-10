import { useEffect, useRef, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import type { Settings, Status } from "../../api";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import { api } from "../api";
import { ChangesSlider } from "../detection/ChangesSlider";
import { Level } from "../detection/DetectionTab";
import { useHelp } from "../shell/Help";

const LEAD_MAX = 500; // Rust `Settings::validate`
const LEAD_STEP = 10;
const field = "rounded-card border border-border bg-card px-3 py-1.5";
const btn = "rounded-card border border-border px-2.5 py-1 hover:bg-card disabled:opacity-40";

function Section(props: {
  icon: IconName;
  title: string;
  hint: string;
  last?: boolean;
  children: ComponentChildren;
}) {
  return (
    <section
      class={`grid grid-cols-[220px_1fr] gap-[18px] py-3.5 ${props.last ? "" : "border-b border-border"}`}
    >
      <div>
        <h3 class="flex items-center gap-2 text-sm font-semibold">
          <Icon name={props.icon} />
          {props.title}
        </h3>
        <p class="mt-1 text-xs text-dim">{props.hint}</p>
      </div>
      <div class="space-y-2">{props.children}</div>
    </section>
  );
}

function Row(props: { label: string; children: ComponentChildren }) {
  return (
    <div class="flex flex-wrap items-center gap-2.5">
      <span class="text-[10px] uppercase tracking-wider text-dim">{props.label}</span>
      {props.children}
    </div>
  );
}

const clampLead = (n: number) => Math.min(LEAD_MAX, Math.max(0, Math.round(n)));

/** Connections and timing. Every change applies at once (no undo). */
export function SetupTab() {
  const [s, setS] = useState<Settings | null>(null);
  const [st, setSt] = useState<Status | null>(null);
  const [midi, setMidi] = useState<string[]>([]);
  const [audio, setAudio] = useState<string[]>([]);
  const [calibrating, setCalibrating] = useState(false);
  const [err, setErr] = useState("");
  const [typed, setTyped] = useState<string | null>(null);
  const on = useRef(false);

  const calibrate = (next: boolean) => {
    if (on.current === next) return;
    on.current = next;
    setCalibrating(next);
    api.calibrateLatency(next).catch(() => setErr("Calibration is not available."));
  };

  useEffect(() => {
    let alive = true;
    const offs: (() => void)[] = [];
    const keep = (u: () => void) => (alive ? offs.push(u) : u());
    // Listen first, then read, so no change is missed.
    api.onStatus((v) => alive && setSt(v)).then(keep, () => {});
    api.onMidiSources((l) => alive && setMidi(l)).then(keep, () => {});
    api.getSettings().then(
      (v) => alive && setS(v),
      () => setErr("Settings unavailable."),
    );
    api.getFullStatus().then(
      (v) => alive && setSt((cur) => cur ?? v),
      () => {},
    );
    api.listMidiSources().then(
      (l) => alive && setMidi((cur) => (cur.length ? cur : l)),
      () => {},
    );
    api.listAudioInputs().then(
      (l) => alive && setAudio(l),
      () => {},
    );
    const off = () => calibrate(false);
    window.addEventListener("blur", off);
    window.addEventListener("beforeunload", off);
    return () => {
      alive = false;
      offs.forEach((u) => u());
      window.removeEventListener("blur", off);
      window.removeEventListener("beforeunload", off);
      off();
    };
  }, []);

  const playing = !!st && st.playing && st.bpm !== null;
  useEffect(() => {
    if (!playing) calibrate(false); // Rust ends it on Stop too
  }, [playing]);

  const helpMidi = useHelp("Pick the app or device that sends the MIDI clock.");
  const helpAudio = useHelp("The input the detector listens to.");
  const helpLead = useHelp("How much earlier the app sends light, in milliseconds.");
  if (!s) return <p class="text-red">{err}</p>;

  const apply = (patch: Partial<Settings>) => {
    const next = { ...s, ...patch };
    setS(next);
    api.saveSettings(next).then(setS, (e) => setErr(String(e)));
  };
  const setLead = (n: number) => {
    setTyped(null);
    apply({ leadMs: clampLead(n) });
  };
  const sources = s.midiSource && !midi.includes(s.midiSource) ? [s.midiSource, ...midi] : midi;
  const level = st?.audio.ok ? st.audio.levelDb : undefined;

  return (
    <div class="max-w-4xl">
      <Section
        icon="midi"
        title="MIDI clock"
        hint="Where the tempo comes from. Deeboo Lighting listens; it never sends."
      >
        <Row label="Input">
          <select
            aria-label="MIDI input"
            class={`${field} min-w-60`}
            value={s.midiSource ?? ""}
            onChange={(e) => apply({ midiSource: e.currentTarget.value || null })}
            {...helpMidi}
          >
            <option value="">Virtual port (Deeboo Lighting In)</option>
            {sources.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
          {playing ? (
            <span class="text-accent">● Receiving · {st.bpm?.toFixed(1)} BPM</span>
          ) : (
            <span class="text-dim">
              {st?.midi.ok ? "○ Connected · waiting for the clock" : "○ Not connected"}
            </span>
          )}
        </Row>
        <p class="text-xs text-dim">
          Any MIDI clock works. The last input used is remembered; new sources appear on their own,
          and if the source goes away it reconnects when it comes back.
        </p>
      </Section>

      <Section
        icon="mic"
        title="Audio input"
        hint="What the detector listens to. Only levels per beat are measured; nothing is recorded."
      >
        <Row label="Device">
          <select
            aria-label="Audio input"
            class={`${field} min-w-60`}
            value={s.audioInput ?? ""}
            onChange={(e) => apply({ audioInput: e.currentTarget.value || null })}
            {...helpAudio}
          >
            <option value="">System default</option>
            {(s.audioInput && !audio.includes(s.audioInput) ? [s.audioInput, ...audio] : audio).map(
              (a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ),
            )}
          </select>
          {level !== undefined && <Level db={level} />}
          <span class={st?.audio.ok ? "text-accent" : "text-dim"}>
            {st?.audio.ok ? "Level OK" : "Not listening"}
          </span>
        </Row>
        {/* shortcut: no reliable permission signal (capture opens only while the clock plays), so
            "allowed" is inferred from a working stream and no System Settings link is offered. */}
        <p class="text-xs text-dim">
          {st?.audio.ok
            ? "Microphone access: allowed"
            : "Microphone access: macOS asks the first time the detector listens. To change it later, open System Settings > Privacy & Security > Microphone."}
        </p>
      </Section>

      <Section
        icon="clock"
        title="Light latency"
        hint="The strip reacts a bit late over Wi-Fi. The app sends this much earlier so light lands on the beat."
      >
        <Row label="Send early by">
          <span class="inline-flex items-center overflow-hidden rounded-card border border-border">
            <button
              type="button"
              aria-label="Send 10 ms later"
              class="px-2.5 py-1 hover:bg-card disabled:opacity-40"
              disabled={s.leadMs <= 0}
              onClick={() => setLead(s.leadMs - LEAD_STEP)}
            >
              −
            </button>
            <input
              aria-label="Send early by (ms)"
              inputMode="numeric"
              class="w-14 bg-bg py-1 text-center"
              value={typed ?? String(s.leadMs)}
              onInput={(e) => setTyped(e.currentTarget.value)}
              onChange={(e) => {
                const n = Number(e.currentTarget.value);
                if (e.currentTarget.value.trim() === "" || Number.isNaN(n)) {
                  setTyped(null);
                  e.currentTarget.value = String(s.leadMs);
                } else setLead(n);
              }}
              {...helpLead}
            />
            <span class="pr-1 text-dim">ms</span>
            <button
              type="button"
              aria-label="Send earlier by 10 ms more"
              class="px-2.5 py-1 hover:bg-card disabled:opacity-40"
              disabled={s.leadMs >= LEAD_MAX}
              onClick={() => setLead(s.leadMs + LEAD_STEP)}
            >
              +
            </button>
          </span>
          <button
            type="button"
            aria-label="Calibrate"
            class={`${btn} ${calibrating ? "border-accent text-accent" : ""}`}
            disabled={!playing && !calibrating}
            onClick={() => calibrate(!calibrating)}
          >
            {calibrating ? "Stop calibrating" : "Calibrate…"}
          </button>
          {!playing && <span class="text-xs text-dim">Start the MIDI clock to calibrate.</span>}
        </Row>
        {calibrating && (
          <div class="flex items-center gap-4 rounded-card border border-border bg-bg p-2.5 text-xs">
            <span aria-hidden="true" class="block h-3.5 w-14 rounded-full bg-text" />
            <div>
              <b>Calibrate:</b> the strip flashes on every beat while the clock plays. Use − / +
              until the flash lands on the kick you hear. 10 ms per click.
            </div>
          </div>
        )}
      </Section>

      <Section
        icon="phrase"
        title="Phrase"
        hint={'How many bars count as one phrase. The counter "phrase: bar x of N" uses it.'}
      >
        <Row label="Bars per phrase">
          <span
            role="radiogroup"
            aria-label="Bars per phrase"
            class="inline-flex overflow-hidden rounded-card border border-border"
          >
            {[4, 8, 16].map((n) => (
              <button
                key={n}
                type="button"
                role="radio"
                aria-checked={s.phraseBars === n}
                class={`px-3 py-1 ${s.phraseBars === n ? "bg-card text-text" : "text-dim"}`}
                onClick={() => apply({ phraseBars: n })}
              >
                {n}
              </button>
            ))}
          </span>
        </Row>
      </Section>

      <Section
        icon="detection"
        title="Moment changes"
        hint="How readily the show moves to Calm or Build-up."
      >
        <ChangesSlider sensitivity={s.sensitivity} onChange={(v) => apply({ sensitivity: v })} />
      </Section>

      <div class="flex items-center gap-3 pt-3.5">
        <button
          type="button"
          aria-label="Quit Deeboo Lighting"
          class={`${btn} text-red`}
          onClick={() => void api.quit()}
        >
          Quit Deeboo Lighting
        </button>
        {err && <span class="text-red">{err}</span>}
      </div>
    </div>
  );
}
