import type { api as realApi, MomentName, Selection, Settings, Status } from "./api";

/** Dev-only (`npm run dev`, open `index.html?mock`); the marker lets the build check prove it is gone from dist. */
export const MOCK_MARKER = "__DEEBOO_POPOVER_MOCK_HARNESS__";

const PALETTES: Selection["palette"][] = [
  {
    name: "Neon",
    colours: [
      { h: 300, s: 1000, v: 1000 },
      { h: 180, s: 1000, v: 1000 },
      { h: 260, s: 600, v: 700 },
    ],
  },
  {
    name: "Sunset",
    colours: [
      { h: 20, s: 1000, v: 1000 },
      { h: 340, s: 1000, v: 1000 },
      { h: 45, s: 700, v: 800 },
    ],
  },
  {
    name: "Ice",
    colours: [
      { h: 200, s: 1000, v: 1000 },
      { h: 0, s: 0, v: 1000 },
      { h: 220, s: 500, v: 600 },
    ],
  },
];
const SETS = ["Club night", "Warm-up", "After hours"];

export function installMock(api: typeof realApi): void {
  console.info(MOCK_MARKER);
  const flags = new URLSearchParams(location.search);
  let settings: Settings = {
    midiSource: null,
    audioInput: null,
    phraseBars: 8,
    sensitivity: 5,
    brightnessPct: 80,
    leadMs: 0,
    lightsOn: true,
    calmKickDb: 16,
    calmHighDb: 12,
    riserDb: 7,
    helpTooltips: true,
  };
  let set = 0;
  let pal = -1; // -1: Auto
  let tick = 0;
  let forced: MomentName | null = null;
  let lightsOn = true;
  const listeners = new Set<(s: Status) => void>();
  const status = (): Status => {
    const noClock = flags.has("noclock");
    const beat = tick % 4;
    const bar = Math.floor(tick / 4) % 8;
    const p = PALETTES[
      pal < 0 ? Math.floor(tick / 32) % PALETTES.length : pal
    ] as Selection["palette"];
    return {
      strip: flags.has("nostrip")
        ? { kind: "notConfigured" }
        : flags.has("striplost")
          ? { kind: "error" }
          : { kind: "connected", name: "dj lights", segments: 8, reported: 8 },
      bpm: noClock ? null : 122,
      playing: !noClock,
      beatInBar: noClock ? null : beat + 1,
      barInPhrase: noClock ? null : bar + 1,
      phraseBars: 8,
      moment: forced ?? "rhythm",
      auto: forced === null,
      predicted: false,
      lightsOn,
      midi: { source: null, ok: true },
      audio: flags.has("audiofail")
        ? { ok: false, device: "Loopback" }
        : { ok: !noClock, device: "Loopback", levelDb: -24 },
      selection: { set: SETS[set] as string, setId: set + 1, paletteAuto: pal < 0, palette: p },
      clip: null,
    };
  };
  const push = () => listeners.forEach((l) => l(status()));
  setInterval(() => {
    tick++;
    push();
  }, 492);

  const sel = (): Selection => status().selection;
  const wrap = (n: number, len: number) => ((n % len) + len) % len;
  Object.assign(api, {
    getStatus: () => Promise.resolve(status()),
    getSettings: () => Promise.resolve({ ...settings }),
    saveSettings: (s: Settings) => Promise.resolve((settings = s)),
    uiCommand: (c: { kind: string; on?: boolean; moment?: MomentName }) => {
      if (c.kind === "lights") lightsOn = Boolean(c.on);
      else forced = c.kind === "force" ? (c.moment ?? null) : null;
      push();
      return Promise.resolve();
    },
    selectSet: (d: number) => {
      set = wrap(set + d, SETS.length);
      push();
      return Promise.resolve(sel());
    },
    selectPalette: (d: number) => {
      pal = wrap(pal + 1 + d, PALETTES.length + 1) - 1;
      push();
      return Promise.resolve(sel());
    },
    openEditor: (section: string) => {
      console.info("openEditor", section);
      return Promise.resolve();
    },
    quit: () => {
      console.info("quit");
      return Promise.resolve();
    },
    setWindowHeight: () => Promise.resolve(),
    onStatus: (cb: (s: Status) => void) => {
      listeners.add(cb);
      return Promise.resolve(() => listeners.delete(cb));
    },
  });
}
