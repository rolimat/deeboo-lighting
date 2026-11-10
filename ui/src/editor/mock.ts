import type { Settings, Status } from "../api";
import type { Library, Sequence, api as realApi } from "./api";
import { fromCells } from "./sequences/cells";

/** Dev-only (`npm run dev`, open `editor.html?mock`); the marker lets the build check prove it is gone from dist. */
export const MOCK_MARKER = "__DEEBOO_EDITOR_MOCK_HARNESS__";

const hsv = (h: number, s = 1000, v = 1000) => ({ h, s, v });
const SEGS = 8;
/** A sequence of `bars` bars from a (segment, half-beat) -> role rule. */
const gen = (
  name: string,
  moment: string,
  bars: number,
  role: (s: number, h: number) => string,
) => {
  const cols = Array.from({ length: bars * 8 }, (_, h) =>
    Array.from({ length: SEGS }, (_, s) => role(s, h)),
  );
  return { name, moment, steps: fromCells(cols) } satisfies Sequence;
};
const PATTERNS: Record<string, (s: number, h: number) => string> = {
  halves: (s, h) => (s < 4 === (Math.floor(h / 2) % 2 === 0) ? "principal" : "contraste"),
  snake: (s, h) => (s === Math.floor(h / 2) % SEGS ? "principal" : "off"),
  drift: (s, h) => (s === Math.floor(h / 4) % SEGS ? "suave" : "off"),
  strobe: (_, h) => (h % 2 === 0 ? "white" : "off"),
};
const halves = (s: number, h: number) =>
  s < 4 === (Math.floor(h / 2) % 2 === 0) ? "principal" : "contraste";
const pat = (p: string) => PATTERNS[p] ?? halves;

export function installMock(api: typeof realApi): void {
  console.info(MOCK_MARKER);
  let lib: Library = {
    schemaVersion: 2,
    shape: {
      pieces: [
        ...[0, 1, 2, 3].map((i) => ({ x: i * 2, y: 3, angle: 0 as const, len: 2 as const })),
        { x: 8, y: 3, angle: 270 as const, len: 2 as const },
        { x: 8, y: 1, angle: 0 as const, len: 2 as const },
        { x: 10, y: 1, angle: 0 as const, len: 2 as const },
        { x: 12, y: 1, angle: 0 as const, len: 2 as const },
      ],
    },
    palettes: [
      [1, { name: "Neon", principal: hsv(300), contraste: hsv(180), suave: hsv(260, 600, 700) }],
      [2, { name: "Sunset", principal: hsv(20), contraste: hsv(340), suave: hsv(45, 700, 800) }],
      [3, { name: "Ice", principal: hsv(200), contraste: hsv(0, 0), suave: hsv(220, 500, 600) }],
      [4, { name: "Acid", principal: hsv(135), contraste: hsv(285), suave: hsv(135, 300, 950) }],
    ],
    sequences: [
      [
        1,
        gen("Pulse", "rhythm", 2, (s, h) =>
          s % 2 === Math.floor(h / 2) % 2 ? "principal" : "off",
        ),
      ],
      [
        2,
        gen("Chase", "rhythm", 2, (s, h) => (s === Math.floor(h / 2) % SEGS ? "contraste" : "off")),
      ],
      [3, gen("Strobe", "rhythm", 1, pat("strobe"))],
      [4, gen("Breathe", "calm", 2, (s, h) => (h < 8 ? "suave" : s % 2 ? "off" : "suave"))],
      [5, gen("Glow", "calm", 1, (s) => (s % 3 === 0 ? "suave" : "off"))],
      [6, gen("Riser", "buildUp", 2, (s, h) => (s <= h / 2 ? "principal" : "off"))],
      [7, gen("Sparkle", "buildUp", 1, (s, h) => ((s * 3 + h) % 5 === 0 ? "white" : "off"))],
    ],
    sets: [
      {
        id: 1,
        name: "Warm-up",
        rhythm: {
          sequences: [{ id: 1 }, { id: 2 }, { pattern: "snake" }],
          order: "inOrder",
          interval: { bars: 8 },
        },
        calm: { sequences: [{ id: 4 }, "generated"], order: "random", interval: "oncePerBreak" },
        buildup: { sequences: [{ id: 6 }, { id: 7 }], order: "inOrder", interval: { bars: 2 } },
        palettes: [1, 2, 3, 4],
        paletteOrder: "inOrder",
        paletteChange: "atDrop",
      },
      {
        id: 2,
        name: "Peak time",
        rhythm: { sequences: [{ id: 3 }, "generated"], order: "random", interval: { bars: 4 } },
        calm: { sequences: [{ id: 5 }], order: "inOrder", interval: { bars: 8 } },
        buildup: { sequences: ["generated"], order: "fixed", interval: { bars: 1 } },
        palettes: [1, 2],
        paletteOrder: "random",
        paletteChange: { bars: 8 },
      },
    ],
    activeSet: 1,
    activePalette: null,
  };
  let settings: Settings = {
    midiSource: null,
    audioInput: null,
    phraseBars: 8,
    sensitivity: 5,
    brightnessPct: 100,
    leadMs: 120,
    lightsOn: true,
    calmKickDb: 15,
    calmHighDb: 12,
    riserDb: 6,
    helpTooltips: true,
  };
  let beat = 0;
  let wired = SEGS;
  const status = (): Status => ({
    strip: { kind: "connected", name: "dj lights", segments: wired, reported: 10 },
    bpm: 122,
    playing: true,
    beatInBar: (beat % 4) + 1,
    barInPhrase: (Math.floor(beat / 4) % 8) + 1,
    phraseBars: 8,
    moment: "rhythm",
    auto: true,
    predicted: false,
    lightsOn: true,
    midi: { source: "Mock clock", ok: true },
    audio: { ok: true },
    clip: (() => {
      const n = lib.sets.find((x) => x.id === lib.activeSet)?.rhythm.sequences.length ?? 1;
      return {
        moment: "rhythm" as const,
        index: Math.floor(beat / 8) % n,
        elapsed: (beat * 2) % 16,
        total: 16,
      };
    })(),
    selection: {
      set: lib.sets.find((x) => x.id === lib.activeSet)?.name ?? "",
      paletteAuto: true,
      palette: { name: "Neon", colours: [hsv(300), hsv(180), hsv(260)] },
    },
  });
  const listeners = new Set<(s: Status) => void>();
  setInterval(() => {
    beat += 1;
    listeners.forEach((cb) => cb(status()));
  }, 500);

  Object.assign(api, {
    getStatus: async () => status().strip,
    getFullStatus: async () => status(),
    onStatus: async (cb: (s: Status) => void) => {
      listeners.add(cb);
      return () => void listeners.delete(cb);
    },
    getLibrary: async () => lib,
    saveLibrary: async (l: Library) =>
      (lib = { ...l, activeSet: lib.activeSet, activePalette: lib.activePalette }),
    // one step of "Strobe" costs two messages, to show the warning
    messageCounts: async (s: Sequence) =>
      s.steps.map((_, i) => (s.name === "Strobe" && i === 1 ? 2 : 1)),
    previewGenerated: async (m: string) =>
      gen(
        "Generated",
        m,
        2,
        m === "calm" ? pat("drift") : m === "buildUp" ? pat("strobe") : pat("snake"),
      ),
    patternSequence: async (p: string) =>
      gen(
        p,
        "rhythm",
        2,
        p === "snake" || p === "drift" || p === "strobe" ? pat(p) : pat("halves"),
      ),
    getSettings: async () => settings,
    saveSettings: async (s: Settings) => (settings = s),
    listMidiSources: async () => ["Mock clock", "IAC Driver Bus 1"],
    listAudioInputs: async () => ["MacBook Microphone", "Mock interface"],
    // Shape only: the mock's sequences keep their 8 roles.
    setWiredSegments: async (n: number) => {
      wired = n;
      const ps = lib.shape.pieces.slice(0, n);
      while (ps.length < n) ps.push({ x: ps.length % 30, y: 8, angle: 0, len: 1 });
      return (lib = { ...lib, shape: { pieces: ps } });
    },
    detectionWatch: async () => {},
    setDetectionThresholds: async () => settings,
    onDetector: async () => () => {},
    onWizard: async () => () => {},
    onSection: async () => () => {},
    wizardCancel: async () => {},
    identifySegment: async () => {},
    previewPalette: async () => {},
    previewSequence: async () => {},
    setLiveSet: async (id: number) => {
      lib = { ...lib, activeSet: id };
      return status().selection;
    },
  } satisfies Partial<typeof realApi>);
}
