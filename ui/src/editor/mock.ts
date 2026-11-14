import type { MomentName, Settings, Status } from "../api";
import type {
  DetectorEvent,
  Library,
  PatternName,
  Sequence,
  Thresholds,
  api as realApi,
} from "./api";
import { fromCells } from "./sequences/cells";
import { PATTERNS as PATTERN_LISTS } from "./sequences/SequencesTab";

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
  return { name, moment, fade: true, steps: fromCells(cols) } satisfies Sequence;
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

/** Fake detector beat on a 64-beat loop: Rhythm, a break (Calm), a riser (Build-up), then the drop. */
function detectorBeat(b: number): DetectorEvent {
  const p = ((b % 64) + 64) % 64;
  const wob = (k: number) => Math.sin(b * 1.7 + k) * 1.5;
  const calm = p >= 24 && p < 40;
  const rise = p >= 40 && p < 56;
  const kickRefDb = -20;
  const subRefDb = -26;
  const highRefDb = -30;
  return {
    kickDb: kickRefDb + wob(0) - (calm ? 28 : rise ? 6 : 0),
    subDb: subRefDb + wob(1) - (calm ? 24 : rise ? 4 : 0),
    highDb: highRefDb + wob(2) - (calm ? 16 : 0) + (rise ? (p - 40) * 0.8 : 0),
    kickRefDb,
    subRefDb,
    highRefDb,
    moment: calm ? "calm" : rise ? "buildUp" : "rhythm",
  };
}

export function installMock(api: typeof realApi): void {
  console.info(MOCK_MARKER);
  let lib: Library = {
    schemaVersion: 3,
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
      [
        1,
        {
          name: "Ocean",
          principal: hsv(190, 1000, 1000),
          contraste: hsv(320, 900, 1000),
          suave: hsv(190, 300, 1000),
        },
      ],
      [
        2,
        {
          name: "Laser",
          principal: hsv(125, 1000, 1000),
          contraste: hsv(325, 1000, 1000),
          suave: hsv(160, 600, 1000),
        },
      ],
      [
        3,
        {
          name: "UV",
          principal: hsv(257, 1000, 1000),
          contraste: hsv(177, 1000, 1000),
          suave: hsv(282, 500, 1000),
        },
      ],
      [
        4,
        {
          name: "Deep sea",
          principal: hsv(197, 1000, 1000),
          contraste: hsv(157, 1000, 1000),
          suave: hsv(182, 400, 1000),
        },
      ],
      [
        5,
        {
          name: "Ember",
          principal: hsv(28, 1000, 1000),
          contraste: hsv(53, 1000, 1000),
          suave: hsv(43, 500, 1000),
        },
      ],
      [
        6,
        {
          name: "Green",
          principal: hsv(130, 1000, 550),
          contraste: hsv(130, 1000, 1000),
          suave: hsv(130, 450, 1000),
        },
      ],
      [
        7,
        {
          name: "Lavender",
          principal: hsv(270, 550, 1000),
          contraste: hsv(220, 450, 1000),
          suave: hsv(220, 250, 1000),
        },
      ],
      [
        8,
        {
          name: "Sorbet",
          principal: hsv(45, 650, 1000),
          contraste: hsv(350, 650, 1000),
          suave: hsv(350, 300, 1000),
        },
      ],
      [
        9,
        {
          name: "Peach and mint",
          principal: hsv(20, 550, 1000),
          contraste: hsv(150, 550, 1000),
          suave: hsv(40, 250, 1000),
        },
      ],
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
          sequences: [{ id: 1 }, { id: 2 }, { pattern: "crossPairs" }],
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
    showInDock: "editor",
    detectorByInput: {},
  };
  let beat = 0;
  let wired = SEGS;
  const noClock = new URLSearchParams(location.search).has("noclock");
  const status = (): Status => ({
    strip: {
      kind: "connected",
      name: "dj lights",
      segments: wired,
      reported: 10,
      ip: "192.168.1.42",
    },
    bpm: noClock ? null : 122,
    playing: !noClock,
    beatInBar: (beat % 4) + 1,
    barInPhrase: (Math.floor(beat / 4) % 8) + 1,
    phraseBars: 8,
    moment: "rhythm",
    auto: true,
    predicted: false,
    lightsOn: true,
    midi: { source: "Mock clock", ok: true },
    audio: { ok: true, device: "MacBook Pro Microphone", levelDb: -18 },
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
      setId: lib.activeSet,
      paletteAuto: true,
      palette: { name: "Neon", colours: [hsv(300), hsv(180), hsv(260)] },
    },
  });
  const listeners = new Set<(s: Status) => void>();
  const sourceListeners = new Set<(l: string[]) => void>();
  let sources = ["Mock clock", "IAC Driver Bus 1"];
  // A source that starts after the app: it appears on its own, with no refresh.
  setTimeout(() => {
    sources = [...sources, "Late Source"];
    sourceListeners.forEach((cb) => cb(sources));
  }, 4000);
  const detectors = new Set<(e: DetectorEvent) => void>();
  setInterval(() => {
    beat += 1;
    listeners.forEach((cb) => cb(status()));
    if (!noClock) detectors.forEach((cb) => cb(detectorBeat(beat)));
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
    patternSequence: async (p: PatternName) => {
      const m = (Object.keys(PATTERN_LISTS) as MomentName[]).find((k) =>
        PATTERN_LISTS[k].some(([n]) => n === p),
      );
      const label = PATTERN_LISTS[m ?? "rhythm"].find(([n]) => n === p)?.[1] ?? p;
      return gen(
        label,
        m ?? "rhythm",
        2,
        pat(m === "calm" ? "drift" : m === "buildUp" ? "strobe" : "snake"),
      );
    },
    getSettings: async () => settings,
    // Mirrors Rust's switch_input: the map is not the UI's; a new input swaps the detector values.
    saveSettings: async (s: Settings) => {
      const key = (i: string | null) => i ?? "";
      const map = { ...settings.detectorByInput };
      let next = { ...s, detectorByInput: map };
      if (s.audioInput !== settings.audioInput) {
        const d = (v: Settings) => ({
          sensitivity: v.sensitivity,
          calmKickDb: v.calmKickDb,
          calmHighDb: v.calmHighDb,
          riserDb: v.riserDb,
        });
        map[key(settings.audioInput)] = d(settings);
        const back = map[key(s.audioInput)] ?? {
          sensitivity: 5,
          calmKickDb: 15,
          calmHighDb: 12,
          riserDb: 6,
        };
        const rest = Object.fromEntries(
          Object.entries(map).filter(([k]) => k !== key(s.audioInput)),
        );
        next = { ...next, ...back, detectorByInput: rest };
      }
      return (settings = next);
    },
    listMidiSources: async () => sources,
    onMidiSources: async (cb: (l: string[]) => void) => {
      sourceListeners.add(cb);
      return () => void sourceListeners.delete(cb);
    },
    calibrateLatency: async () => {},
    quit: async () => console.info("mock: quit"),
    listAudioInputs: async () => ["MacBook Microphone", "Mock interface"],
    // Shape only: the mock's sequences keep their 8 roles.
    setWiredSegments: async (n: number) => {
      wired = n;
      const ps = lib.shape.pieces.slice(0, n);
      while (ps.length < n) ps.push({ x: ps.length % 30, y: 8, angle: 0, len: 1 });
      return (lib = { ...lib, shape: { pieces: ps } });
    },
    detectionWatch: async () => {},
    setDetectionThresholds: async (t: Thresholds) => (settings = { ...settings, ...t }),
    onDetector: async (cb: (e: DetectorEvent) => void) => {
      detectors.add(cb);
      // Replay the last 8 bars so the timeline is not empty while it fills.
      if (!noClock) for (let b = beat - 31; b <= beat; b++) cb(detectorBeat(b));
      return () => void detectors.delete(cb);
    },
    onWizard: async () => () => {},
    onSection: async () => () => {},
    wizardCancel: async () => {},
    wizardState: async () => ({ kind: "idle" as const }),
    identifySegment: async () => {},
    previewPalette: async () => {},
    previewSequence: async () => {},
    setLiveSet: async (id: number) => {
      lib = { ...lib, activeSet: id };
      return status().selection;
    },
  } satisfies Partial<typeof realApi>);
}
