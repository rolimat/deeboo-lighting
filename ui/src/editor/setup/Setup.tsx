import { useEffect, useState } from "preact/hooks";
import type { Settings } from "../../api";
import { api } from "../api";
import { useHelp } from "../shell/Help";

const SYSTEM_DEFAULT = "";

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint: string;
  children: preact.ComponentChildren;
}) {
  return (
    <label class="flex items-center gap-4" {...useHelp(hint)}>
      <span class="w-44 text-dim">{label}</span>
      {children}
    </label>
  );
}

/** Settings that apply at once (no undo). */
export function Setup() {
  const [s, setS] = useState<Settings | null>(null);
  const [midi, setMidi] = useState<string[]>([]);
  const [audio, setAudio] = useState<string[]>([]);
  const [err, setErr] = useState("");
  useEffect(() => {
    api.getSettings().then(setS, (e) => setErr(String(e)));
    api.listMidiSources().then(setMidi, () => {});
    api.listAudioInputs().then(setAudio, () => {});
  }, []);
  if (!s) return <p class="text-red">{err}</p>;
  const apply = (patch: Partial<Settings>) => {
    const next = { ...s, ...patch };
    setS(next);
    api.saveSettings(next).then(setS, (e) => setErr(String(e)));
  };
  const field = "rounded-card border border-border bg-card px-3 py-2";
  return (
    <form class="max-w-xl space-y-4" onSubmit={(e) => e.preventDefault()}>
      <Row label="MIDI input" hint="Where the clock comes from.">
        <select
          class={field}
          value={s.midiSource ?? SYSTEM_DEFAULT}
          onChange={(e) => apply({ midiSource: e.currentTarget.value || null })}
        >
          <option value={SYSTEM_DEFAULT}>Virtual port (Deeboo Lighting In)</option>
          {midi.map((m) => (
            <option key={m}>{m}</option>
          ))}
        </select>
      </Row>
      <Row label="Audio input" hint="The input the detector listens to.">
        <select
          class={field}
          value={s.audioInput ?? SYSTEM_DEFAULT}
          onChange={(e) => apply({ audioInput: e.currentTarget.value || null })}
        >
          <option value={SYSTEM_DEFAULT}>System default</option>
          {audio.map((a) => (
            <option key={a}>{a}</option>
          ))}
        </select>
      </Row>
      <Row label="Light latency (ms)" hint="Send lights earlier to match the delay of the strip.">
        <input
          type="number"
          min={0}
          max={500}
          class={field}
          value={s.leadMs}
          onChange={(e) => apply({ leadMs: Number(e.currentTarget.value) })}
        />
      </Row>
      <Row label="Phrase length" hint="Bars in a phrase.">
        <select
          class={field}
          value={s.phraseBars}
          onChange={(e) => apply({ phraseBars: Number(e.currentTarget.value) })}
        >
          {[4, 8, 16].map((n) => (
            <option key={n}>{n}</option>
          ))}
        </select>
      </Row>
      <Row label="Detection sensitivity" hint="Higher reacts to quieter changes.">
        <input
          type="range"
          min={1}
          max={10}
          value={s.sensitivity}
          onChange={(e) => apply({ sensitivity: Number(e.currentTarget.value) })}
        />
      </Row>
      {err && <p class="text-red">{err}</p>}
    </form>
  );
}
