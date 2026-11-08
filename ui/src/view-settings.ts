import { api, type Settings } from "./api";
import { el } from "./dom";

let tail: Promise<unknown> = Promise.resolve();

/** Saves run one at a time, each on the latest stored settings, so quick changes never overwrite each other. */
export function queueSave(change: (s: Settings) => Settings): Promise<Settings> {
  const run = tail.then(async () => api.saveSettings(change(await api.getSettings())));
  tail = run.catch(() => {});
  return run;
}

function select(label: string, first: string, names: string[], current: string | null) {
  const s = el("select");
  s.setAttribute("aria-label", label);
  s.append(new Option(first, ""), ...names.map((n) => new Option(n, n)));
  s.value = current ?? "";
  return s;
}

function field(title: string, control: HTMLElement): HTMLElement {
  const f = el("label", "field");
  f.append(el("span", "field-title", title), control);
  return f;
}

function range(label: string, min: number, max: number, value: number): HTMLInputElement {
  const r = el("input");
  r.type = "range";
  r.min = String(min);
  r.max = String(max);
  r.value = String(value);
  r.setAttribute("aria-label", label);
  return r;
}

export function mountSettings(onBack: () => void): { node: HTMLElement; open: () => void } {
  const node = el("div", "view-settings");
  const error = el("div", "error");
  error.hidden = true;
  const back = el("button", "back", "‹ Settings");
  back.setAttribute("aria-label", "Back");
  back.onclick = onBack;
  const body = el("div", "card settings-body");
  node.append(back, body, error);

  const save = (change: (s: Settings) => Settings) => {
    queueSave(change).then(
      () => (error.hidden = true),
      (e: unknown) => show(e),
    );
  };
  const show = (e: unknown) => {
    error.textContent = String(e);
    error.hidden = false;
  };

  const open = () => {
    Promise.all([api.getSettings(), api.listMidiSources(), api.listAudioInputs()]).then(
      ([s, midi, audio]) => {
        const midiSel = select("MIDI input", "Deeboo Lighting In only", midi, s.midiSource);
        midiSel.onchange = () => save((c) => ({ ...c, midiSource: midiSel.value || null }));
        const audioSel = select("Audio input", "System default", audio, s.audioInput);
        audioSel.onchange = () => save((c) => ({ ...c, audioInput: audioSel.value || null }));

        const seg = el("div", "segmented");
        const segButtons = [4, 8, 16].map((n) => {
          const b = el("button", "", String(n));
          b.setAttribute("aria-label", `${n} bars`);
          b.setAttribute("aria-pressed", String(n === s.phraseBars));
          b.onclick = () => {
            segButtons.forEach((o) => o.setAttribute("aria-pressed", String(o === b)));
            save((c) => ({ ...c, phraseBars: n }));
          };
          return b;
        });
        seg.append(...segButtons);

        const sens = range("Detection sensitivity", 1, 10, s.sensitivity);
        sens.onchange = () => save((c) => ({ ...c, sensitivity: Number(sens.value) }));
        const lat = range("Light latency", 0, 500, s.leadMs);
        lat.step = "10";
        lat.onchange = () => save((c) => ({ ...c, leadMs: Number(lat.value) }));

        const quit = el("button", "quit", "Quit");
        quit.setAttribute("aria-label", "Quit");
        quit.onclick = () => void api.quit();
        body.replaceChildren(
          field("MIDI input", midiSel),
          field("Audio input", audioSel),
          field("Phrase length", seg),
          field("Detection sensitivity", sens),
          field("Light latency (ms)", lat),
          quit,
        );
      },
      show,
    );
  };
  return { node, open };
}
