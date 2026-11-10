import {
  api,
  type Hsv,
  type MomentName,
  type Selection,
  type Status,
  type StripStatus,
} from "./api";
import { el } from "./dom";
import { icons, type IconName } from "./icons";

const MOMENTS: [MomentName, string, IconName][] = [
  ["rhythm", "Rhythm", "rhythm"],
  ["calm", "Calm", "calm"],
  ["buildUp", "Build-up", "buildup"],
];

/** Bundled static SVG markup (never user data). */
function icon(name: IconName): HTMLSpanElement {
  const s = el("span", "icon");
  s.setAttribute("aria-hidden", "true");
  s.innerHTML = icons[name].trim();
  return s;
}

/** Writes only on change, so text under the pointer is never rewritten. */
function setText(n: HTMLElement, t: string): void {
  if (n.textContent !== t) n.textContent = t;
}

function setAttr(n: HTMLElement, name: string, v: string): void {
  if (n.getAttribute(name) !== v) n.setAttribute(name, v);
}

function iconButton(label: string, name: IconName, onClick: () => void): HTMLButtonElement {
  const b = el("button", "icon-btn");
  b.setAttribute("aria-label", label);
  b.setAttribute("aria-pressed", "false");
  b.append(icon(name));
  b.onclick = onClick;
  return b;
}

/** CSS colour for a strip colour (H 0-360, S and V 0-1000). */
function hsvCss({ h, s, v }: Hsv): string {
  const sat = s / 1000;
  const val = (v / 1000) * 255;
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round(val * (1 - sat * Math.max(0, Math.min(k, 4 - k, 1))));
  };
  return `rgb(${f(5)}, ${f(3)}, ${f(1)})`;
}

/** `title ‹ name ›`, for stepping through a list. */
function stepper(kind: string, title: string, step: (delta: -1 | 1) => void) {
  const row = el("div", `row row-${kind}`);
  const name = el("span", "row-text");
  const prev = el("button", "step", "‹");
  prev.setAttribute("aria-label", `Previous ${kind}`);
  prev.onclick = () => step(-1);
  const next = el("button", "step", "›");
  next.setAttribute("aria-label", `Next ${kind}`);
  next.onclick = () => step(1);
  const box = el("div", "stepper");
  box.append(prev, name, next);
  row.append(el("span", "field-title", title), box);
  return { row, name, box };
}

function stripName(s: StripStatus): string {
  switch (s.kind) {
    case "notConfigured":
      return "not connected";
    case "connecting":
      return "connecting…";
    case "connected":
      return `${s.name} · connected`;
    case "error":
      return "connection error";
    case "keyInvalid":
      return "key no longer valid";
    case "notFound":
      return "not found";
  }
}

type Dot = { state: "ok" | "off" | "bad"; help: string };

function clockDot(s: Status): Dot {
  return s.playing || s.bpm !== null
    ? { state: "ok", help: "Receiving MIDI clock." }
    : { state: "off", help: "No MIDI clock — start playback in your clock source." };
}

function audioDot(s: Status): Dot {
  if (s.audio.ok) return { state: "ok", help: "Listening to the music." };
  return s.audio.device
    ? { state: "bad", help: "Audio capture failed — check the input in Setup." }
    : { state: "off", help: "Audio is off — it starts when the music plays." };
}

function stripDot(s: Status): Dot {
  switch (s.strip.kind) {
    case "connected":
      return { state: "ok", help: "Strip connected." };
    case "notConfigured":
      return { state: "off", help: "No strip yet — connect one in Setup." };
    case "connecting":
      return { state: "off", help: "Connecting to the strip…" };
    case "keyInvalid":
      return { state: "bad", help: "The strip's key is no longer valid — reconnect it in Setup." };
    case "notFound":
      return {
        state: "bad",
        help: "Strip not found — check its power and Wi-Fi, then open Setup.",
      };
    case "error":
      return { state: "bad", help: "Strip connection lost — check its power and Wi-Fi." };
  }
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

  // Header: tempo and moment
  const bpm = el("div", "bpm", "—");
  const beat = el("div");
  const phrase = el("div");
  const sub = el("div", "tempo-sub");
  sub.append(beat, phrase);
  const chip = el("div", "moment-chip");
  const chipLabel = el("span");
  let chipMoment: MomentName | null = null;
  const header = el("div", "header-row");
  header.append(bpm, sub, el("span", "grow"), chip);

  // Lights
  let lightsOn = true;
  const play = iconButton("Lights", "play", () =>
    run(api.uiCommand({ kind: "lights", on: !lightsOn })),
  );
  play.classList.replace("icon-btn", "play-btn");
  const lightsState = el("div", "lights-state");
  const lightsText = el("div");
  lightsText.append(el("b", "", "Lights"), lightsState);
  const stripLabel = el("span", "strip-name");
  const lights = el("div", "lights");
  lights.append(play, lightsText, el("span", "grow"), stripLabel);

  // Brightness
  const slider = el("input");
  slider.type = "range";
  slider.min = "10";
  slider.max = "100";
  slider.setAttribute("aria-label", "Brightness");
  // Each save reads the stored settings first, so it never overwrites a change made elsewhere.
  slider.onchange = () =>
    void run(
      api
        .getSettings()
        .then((s) => api.saveSettings({ ...s, brightnessPct: Number(slider.value) })),
    );
  void run(api.getSettings().then((s) => (slider.value = String(s.brightnessPct))));
  const brightness = el("div", "row");
  brightness.append(el("span", "field-title", "Brightness"), slider);

  // Moment
  const auto = el("button", "auto", "Auto");
  auto.setAttribute("aria-label", "Auto");
  auto.setAttribute("aria-pressed", "false");
  auto.onclick = () => void run(api.uiCommand({ kind: "auto" }));
  const momentBtns = MOMENTS.map(([m, label, ic]) => ({
    m,
    b: iconButton(label, ic, () => run(api.uiCommand({ kind: "force", moment: m }))),
  }));
  const seg = el("div", "segmented");
  seg.append(auto, ...momentBtns.map((x) => x.b));
  const moment = el("div", "row");
  moment.append(el("span", "field-title", "Moment"), seg);

  // Set and palette
  const act = (p: Promise<Selection>) => void run(p.then((sel) => showSel(sel)));
  const setRow = stepper("set", "Set", (d) => act(api.selectSet(d)));
  const palRow = stepper("palette", "Palette", (d) => act(api.selectPalette(d)));
  const swatches = [0, 1, 2].map(() => el("span", "swatch"));
  const sw = el("span", "swatches");
  sw.append(...swatches);
  palRow.box.insertBefore(sw, palRow.name);
  const showSel = (sel: Selection) => {
    setText(setRow.name, sel.set);
    setText(palRow.name, sel.paletteAuto ? "Auto" : sel.palette.name);
    swatches.forEach((x, i) => {
      const c = hsvCss(sel.palette.colours[i] as Hsv);
      if (x.style.backgroundColor !== c) x.style.backgroundColor = c;
    });
  };

  // Footer: status dots, editor, quit
  const dotBtn = (name: string) => {
    const b = el("button", "dot-btn");
    const dot = el("span", "dot");
    b.append(dot, el("span", "", name));
    b.onclick = () => void run(api.openEditor("setup"));
    return { b, dot, name };
  };
  const dots = [dotBtn("Clock"), dotBtn("Audio"), dotBtn("Strip")];
  const openEditor = el("button", "open-editor", "Open editor");
  openEditor.setAttribute("aria-label", "Open editor");
  openEditor.onclick = () => void run(api.openEditor("sets"));
  const quit = el("button", "quit");
  quit.setAttribute("aria-label", "Quit");
  quit.title = "Quit";
  quit.append(icon("power"));
  quit.onclick = () => void run(api.quit());
  const footer = el("div", "footer");
  footer.append(...dots.map((d) => d.b), el("span", "grow"), openEditor, quit);

  const body = el("div", "body");
  body.append(lights, brightness, moment, setRow.row, palRow.row);
  root.replaceChildren(header, body, footer, error);

  return {
    update(s) {
      setText(bpm, s.bpm === null ? "—" : s.bpm.toFixed(1));
      setText(beat, `BPM · beat ${s.beatInBar ?? "—"}`);
      setText(phrase, `phrase: bar ${s.barInPhrase ?? "—"} of ${s.phraseBars}`);
      if (chipMoment !== s.moment) {
        chipMoment = s.moment;
        const [, label, ic] =
          MOMENTS.find(([m]) => m === s.moment) ?? (MOMENTS[0] as (typeof MOMENTS)[number]);
        chipLabel.textContent = label;
        chip.replaceChildren(icon(ic), chipLabel);
      }

      lightsOn = s.lightsOn;
      setAttr(play, "aria-pressed", String(s.lightsOn));
      setText(lightsState, s.lightsOn ? "On · following the music" : "Off");
      setText(stripLabel, stripName(s.strip));
      setAttr(auto, "aria-pressed", String(s.auto));
      momentBtns.forEach(({ m, b }) =>
        setAttr(b, "aria-pressed", String(!s.auto && m === s.moment)),
      );
      showSel(s.selection);

      [clockDot(s), audioDot(s), stripDot(s)].forEach((d, i) => {
        const { b, dot, name } = dots[i] as (typeof dots)[number];
        setAttr(dot, "data-state", d.state);
        setAttr(b, "title", d.help);
        setAttr(b, "aria-label", `${name}: ${d.help}`);
      });
    },
  };
}
