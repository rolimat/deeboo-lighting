import { api, type MomentName, type Status } from "./api";
import { el, glyphButton } from "./dom";
import { mountSettings, queueSave, stripText } from "./view-settings";

const MOMENTS: [MomentName, string][] = [
  ["rhythm", "Rhythm"],
  ["calm", "Calm"],
  ["buildUp", "Build-up"],
];

/** Writes only on change, so text under the pointer is never rewritten. */
function setText(n: HTMLElement, t: string): void {
  if (n.textContent !== t) n.textContent = t;
}

function pressable(label: string, text: string, onClick: () => void): HTMLButtonElement {
  const b = el("button", "", text);
  b.setAttribute("aria-label", label);
  b.setAttribute("aria-pressed", "false");
  b.onclick = onClick;
  return b;
}

function setPressed(b: HTMLElement, on: boolean): void {
  const v = String(on);
  if (b.getAttribute("aria-pressed") !== v) b.setAttribute("aria-pressed", v);
}

function statusRow(kind: string) {
  const row = el("div", `row row-${kind}`);
  const dot = el("span", "dot");
  const label = el("span", "row-text");
  row.append(dot, label);
  return { row, dot, label };
}

export function mountPopover(root: HTMLElement): { update: (status: Status) => void } {
  const error = el("div", "error");
  error.hidden = true;
  const run = (p: Promise<unknown>) =>
    p.then(
      () => (error.hidden = true),
      (e: unknown) => {
        error.textContent = String(e);
        error.hidden = false;
      },
    );

  // Header
  const gear = glyphButton("gear", "⚙");
  gear.setAttribute("aria-label", "Settings");
  const header = el("div", "header-row");
  header.append(el("h1", "header", "Deeboo Lighting"), gear);

  // Tempo
  const bpm = el("div", "bpm", "—");
  const beats = [1, 2, 3, 4].map((n) => el("span", n === 1 ? "beat accent" : "beat"));
  const bar = el("div", "bar");
  const beatBox = el("div", "beats");
  beatBox.append(...beats);
  const tempo = el("div", "card tempo");
  tempo.append(bpm, beatBox, bar);

  // Lights
  const lightsOn = pressable("Lights on", "Lights ▶", () =>
    run(api.uiCommand({ kind: "lights", on: true })),
  );
  const lightsOff = pressable("Lights off", "Off", () =>
    run(api.uiCommand({ kind: "lights", on: false })),
  );
  const lights = el("div", "segmented");
  lights.append(lightsOn, lightsOff);

  // Moment
  const momentBtns = MOMENTS.map(([m, label]) => ({
    m,
    b: pressable(label, label, () => run(api.uiCommand({ kind: "force", moment: m }))),
  }));
  const auto = pressable("Auto", "Auto", () => run(api.uiCommand({ kind: "auto" })));
  auto.classList.add("auto");
  const predicted = el("span", "predicted", "drop predicted");
  predicted.hidden = true;
  const momentRow = el("div", "segmented");
  momentRow.append(...momentBtns.map((x) => x.b));
  const moment = el("div", "card moment");
  moment.append(momentRow, el("div", "moment-foot"));
  moment.lastElementChild?.append(auto, predicted);

  // Brightness
  const slider = el("input");
  slider.type = "range";
  slider.min = "10";
  slider.max = "100";
  slider.setAttribute("aria-label", "Brightness");
  slider.onchange = () =>
    void run(queueSave((s) => ({ ...s, brightnessPct: Number(slider.value) })));
  void run(api.getSettings().then((s) => (slider.value = String(s.brightnessPct))));
  const brightness = el("div", "card field-row");
  brightness.append(el("span", "field-title", "Brightness"), slider);

  // Status
  const midi = statusRow("midi");
  const strip = statusRow("strip");
  const reconnect = el("button", "row-action", "Reconnect…");
  reconnect.setAttribute("aria-label", "Reconnect strip");
  reconnect.hidden = true;
  reconnect.onclick = () => void run(api.openEditor("strip"));
  strip.row.append(reconnect);
  const audio = statusRow("audio");
  const levelFill = el("span", "level-fill");
  const level = el("span", "level");
  level.append(levelFill);
  audio.row.append(level);
  const statusCard = el("div", "card status");
  statusCard.append(midi.row, strip.row, audio.row);

  const main = el("div", "view-main");
  main.append(tempo, lights, moment, brightness, statusCard);

  let stripStatus: Status["strip"] = { kind: "notConfigured" };
  const settings = mountSettings(() => {
    settings.node.hidden = true;
    main.hidden = false;
  });
  settings.node.hidden = true;
  gear.onclick = () => {
    main.hidden = true;
    settings.node.hidden = false;
    settings.open(stripStatus);
  };

  root.replaceChildren(header, main, settings.node, error);

  const dot = (d: HTMLElement, state: "ok" | "off" | "bad") => {
    if (d.dataset.state !== state) d.dataset.state = state;
  };

  return {
    update(s) {
      setText(bpm, s.bpm === null ? "—" : s.bpm.toFixed(1));
      beats.forEach((b, i) => b.classList.toggle("lit", s.beatInBar === i + 1));
      setText(bar, `bar ${s.barInPhrase ?? "—"} / ${s.phraseBars}`);
      setPressed(lightsOn, s.lightsOn);
      setPressed(lightsOff, !s.lightsOn);
      momentBtns.forEach(({ m, b }) => setPressed(b, m === s.moment));
      setPressed(auto, s.auto);
      setText(auto, s.auto ? "Auto ✓" : "Auto");
      predicted.hidden = !s.predicted;

      setText(midi.label, `MIDI · ${s.midi.source ?? "Deeboo Lighting In"}`);
      dot(midi.dot, s.midi.ok ? "ok" : "bad");
      setText(strip.label, `Strip · ${stripText(s.strip)}`);
      stripStatus = s.strip;
      const k = s.strip.kind;
      dot(
        strip.dot,
        k === "connected" ? "ok" : k === "notConfigured" || k === "connecting" ? "off" : "bad",
      );
      reconnect.hidden = k !== "keyInvalid" && k !== "notFound";
      const a = s.audio;
      setText(audio.label, `Audio · ${a.ok ? (a.device ?? "on") : a.device ? "no access" : "off"}`);
      dot(audio.dot, a.ok ? "ok" : a.device ? "bad" : "off");
      level.hidden = !a.ok;
      // -60 dBFS (silence) to 0 dBFS (clip) across the bar.
      const pct = Math.round(Math.min(1, Math.max(0, ((a.levelDb ?? -60) + 60) / 60)) * 100);
      if (levelFill.style.width !== `${pct}%`) levelFill.style.width = `${pct}%`;
    },
  };
}
