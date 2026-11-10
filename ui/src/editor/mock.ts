import type { Settings, Status } from "../api";
import type { Library, Sequence, api as realApi } from "./api";

/** Dev-only (`npm run dev`, open `editor.html?mock`); the marker lets the build check prove it is gone from dist. */
export const MOCK_MARKER = "__DEEBOO_EDITOR_MOCK_HARNESS__";

const hsv = (h: number, s = 1000, v = 1000) => ({ h, s, v });
const seq = (name: string, moment: string, rows: string[][]): Sequence => ({
  name,
  moment,
  steps: rows.map((roles) => ({ len: "one", roles })),
});
const off = Array<string>(8).fill("off");

export function installMock(api: typeof realApi): void {
  console.info(MOCK_MARKER);
  let lib: Library = {
    schemaVersion: 2,
    shape: {
      pieces: [0, 1, 2, 3].map((i) => ({ x: i * 2, y: 0, angle: 0 as const, len: 2 as const })),
    },
    palettes: [
      [1, { name: "Neon", principal: hsv(300), contraste: hsv(180), suave: hsv(260, 600, 700) }],
      [2, { name: "Sunset", principal: hsv(20), contraste: hsv(340), suave: hsv(45, 700, 800) }],
      [3, { name: "Ice", principal: hsv(200), contraste: hsv(0, 0), suave: hsv(220, 500, 600) }],
    ],
    sequences: [
      [1, seq("Pulse", "rhythm", [["principal", ...off.slice(1)], off])],
      [2, seq("Strobe", "rhythm", [Array(8).fill("white"), off])],
      [3, seq("Drift", "calm", [["suave", "suave", ...off.slice(2)], off])],
      [4, seq("Rise", "buildUp", [["principal", "contraste", ...off.slice(2)], off])],
    ],
    sets: [1, 2].map((id) => {
      const m = {
        sequences: [{ id }, "generated" as const],
        order: "inOrder" as const,
        interval: { bars: 4 },
      };
      return {
        id,
        name: id === 1 ? "Warm-up" : "Peak time",
        rhythm: m,
        calm: { ...m, sequences: [{ id: 3 }] },
        buildup: { ...m, sequences: [{ id: 4 }] },
        palettes: [1, 2],
        paletteOrder: "inOrder" as const,
        paletteChange: "atDrop" as const,
      };
    }),
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
  const status = (): Status => ({
    strip: { kind: "connected", name: "Mock strip", segments: 8 },
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
    selection: {
      set: "Warm-up",
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
    messageCounts: async (s: Sequence) => s.steps.map(() => 1),
    previewGenerated: async () => lib.sequences[0]?.[1] as Sequence,
    patternSequence: async () => lib.sequences[1]?.[1] as Sequence,
    getSettings: async () => settings,
    saveSettings: async (s: Settings) => (settings = s),
    listMidiSources: async () => ["Mock clock", "IAC Driver Bus 1"],
    listAudioInputs: async () => ["MacBook Microphone", "Mock interface"],
    setWiredSegments: async () => lib,
    detectionWatch: async () => {},
    setDetectionThresholds: async () => settings,
    onDetector: async () => () => {},
    onWizard: async () => () => {},
    wizardCancel: async () => {},
    identifySegment: async () => {},
  } satisfies Partial<typeof realApi>);
}
