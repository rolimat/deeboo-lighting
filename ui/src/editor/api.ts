import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { StripStatus, Status, Settings, MomentName, Selection } from "../api";

export interface QrMatrix {
  size: number;
  /** Row-major, `true` = dark. The only cloud-derived secret that crosses IPC; never logged. */
  modules: boolean[];
}

export interface DeviceDto {
  id: string;
  name: string;
  product: string;
  online: boolean;
  supported: boolean;
  /** Why it can't be chosen, when its data is the cause. */
  reason?: string | null;
}

export type WizardStep = "login" | "devices" | "search" | "segments" | "save";

/** Mirrors `WizardState` in `src-tauri/src/wizard/state.rs`. */
export type WizardState =
  | { kind: "idle" }
  | { kind: "waitingScan"; qr: QrMatrix; expiresAtUs: number }
  | { kind: "chooseStrip"; devices: DeviceDto[] }
  | { kind: "searching"; device: DeviceDto; sinceUs: number; help: boolean }
  | { kind: "listening"; device: DeviceDto; sinceUs: number }
  | { kind: "identify"; device: DeviceDto; segments: number }
  | { kind: "saved"; device: DeviceDto; segments: number }
  | { kind: "failed"; step: WizardStep; message: string };

export type PatternName =
  | "halves"
  | "endsCentre"
  | "oddEven"
  | "quarters"
  | "snake"
  | "mirror"
  | "drift"
  | "sparse"
  | "groupFlash"
  | "fastSnake"
  | "strobe";
export type Interval = { bars: number } | "oncePerBreak";
export type SeqRef = { id: number } | "generated" | { pattern: PatternName };
export type PaletteChange = "atDrop" | { bars: number };
export type Order = "inOrder" | "random" | "fixed";
export interface Piece {
  x: number;
  y: number;
  /** Degrees clockwise, a multiple of 15 below 360. */
  angle: number;
  /** Cells, 1 to 4. */
  len: number;
}
export interface Hsv {
  h: number;
  s: number;
  v: number;
}
export interface Palette {
  name: string;
  principal: Hsv;
  contraste: Hsv;
  suave: Hsv;
}
export interface Step {
  len: string;
  roles: string[];
}
export interface Sequence {
  name: string;
  moment: string;
  steps: Step[];
}
export interface MomentSet {
  sequences: SeqRef[];
  order: Order;
  interval: Interval;
}
export interface LibrarySet {
  id: number;
  name: string;
  rhythm: MomentSet;
  calm: MomentSet;
  buildup: MomentSet;
  palettes: number[];
  paletteOrder: Order;
  paletteChange: PaletteChange;
}
/**
 * Mirrors `light_core::library::Library`. Ids are assigned by Rust: an id the saved library does not
 * have is a client-side handle for a new entry (use any unused number, e.g. a counter from 1_000_000);
 * Rust gives it the next free id and rewrites every reference to it. `saveLibrary` returns the
 * library with the real ids.
 */
export interface Library {
  schemaVersion: 2;
  shape: { pieces: Piece[] };
  palettes: [number, Palette][];
  sequences: [number, Sequence][];
  sets: LibrarySet[];
  activeSet: number;
  activePalette: number | null;
}

export interface Thresholds {
  calmKickDb: number;
  calmHighDb: number;
  riserDb: number;
}
/** Mirrors `DetectorEvent` in `src-tauri/src/runtime.rs`: one beat, band levels in dB. */
export interface DetectorEvent {
  kickDb: number;
  subDb: number;
  highDb: number;
  kickRefDb: number;
  subRefDb: number;
  highRefDb: number;
  moment: MomentName;
}

/** The editor's only door to Rust; components never import `@tauri-apps/api`. */
export const api = {
  getStatus: async (): Promise<StripStatus> => (await invoke<Status>("get_status")).strip,
  getLibrary: (): Promise<Library> => invoke("get_library"),
  /** Rejects with a short message; the previous library is kept. */
  saveLibrary: (library: Library): Promise<Library> => invoke("save_library", { library }),
  /** Messages each step costs to reach from the previous one (the first wraps from the last). */
  messageCounts: (sequence: Sequence, paletteId: number): Promise<number[]> =>
    invoke("message_counts", { sequence, paletteId }),
  /** An example of what Generated plays for a moment, built for the saved shape with a fresh seed. */
  previewGenerated: (moment: MomentName): Promise<Sequence> =>
    invoke("preview_generated", { moment }),
  /** A pattern as an editable sequence for the strip's shape (Duplicate to edit) or a read-only preview. */
  patternSequence: (pattern: PatternName, seed: number): Promise<Sequence> =>
    invoke("pattern_sequence", { pattern, seed }),
  /** "Show on strip": plays `sequence` (a library id) in `palette` on the real strip until switched off. */
  previewPalette: (on: boolean, palette: Palette, sequence: number): Promise<void> =>
    invoke("preview_palette", { on, palette, sequence }),
  /** Makes a set live (the engine, not the library); resolves to the new selection. */
  setLiveSet: (id: number): Promise<Selection> => invoke("set_live_set", { id }),
  /** Hold-to-preview: plays one set clip in `palette` on the real strip until switched off. */
  previewSequence: (
    on: boolean,
    palette: Palette,
    sequence: SeqRef,
    moment: MomentName,
  ): Promise<void> => invoke("preview_sequence", { on, palette, sequence, moment }),
  wizardStart: (userCode: string): Promise<void> => invoke("wizard_start", { userCode }),
  wizardChoose: (deviceId: string): Promise<void> => invoke("wizard_choose", { deviceId }),
  wizardRetry: (): Promise<void> => invoke("wizard_retry"),
  wizardCancel: (): Promise<void> => invoke("wizard_cancel"),
  stripIdentify: (segment: number): Promise<void> => invoke("strip_identify", { segment }),
  /** Flashes a segment (1-based) through the app's own strip connection; rejects with a short message. */
  identifySegment: (segment: number): Promise<void> => invoke("identify_segment", { segment }),
  wizardSave: (wired: number): Promise<void> => invoke("wizard_save", { wired }),
  /** Changes how many segments are wired (1..=what the controller reports); returns the resized library. */
  setWiredSegments: (n: number): Promise<Library> => invoke("set_wired_segments", { n }),
  getSettings: (): Promise<Settings> => invoke("get_settings"),
  /** Applies at once (no undo); resolves to the stored settings. */
  saveSettings: (settings: Settings): Promise<Settings> => invoke("save_settings", { settings }),
  listMidiSources: (): Promise<string[]> => invoke("list_midi_sources"),
  listAudioInputs: (): Promise<string[]> => invoke("list_audio_inputs"),
  /** The OS's MIDI sources changed (event `midi-sources`; no polling). */
  onMidiSources: (cb: (sources: string[]) => void): Promise<UnlistenFn> =>
    listen<string[]>("midi-sources", (e) => cb(e.payload)),
  /** Latency calibration: the strip flashes white on every beat while the clock plays, until `false`. */
  calibrateLatency: (on: boolean): Promise<void> => invoke("calibrate_latency", { on }),
  quit: (): Promise<void> => invoke("quit"),
  /** The live readout; the backend pushes it on every change (event `status`). */
  getFullStatus: (): Promise<Status> => invoke("get_status"),
  /** The editor was already open and was asked to show another tab (event `editor-section`). */
  onSection: (cb: (section: string) => void): Promise<UnlistenFn> =>
    listen<string>("editor-section", (e) => cb(e.payload)),
  onStatus: (cb: (status: Status) => void): Promise<UnlistenFn> =>
    listen<Status>("status", (e) => cb(e.payload)),
  /** The editor's Detection section is open (`true`) or closed: `detector` events flow only while open. */
  detectionWatch: (on: boolean): Promise<void> => invoke("detection_watch", { on }),
  /** Whole dB; rejects with a short message when out of range. */
  setDetectionThresholds: (thresholds: Thresholds): Promise<Settings> =>
    invoke("set_detection_thresholds", { thresholds }),
  onDetector: (cb: (e: DetectorEvent) => void): Promise<UnlistenFn> =>
    listen<DetectorEvent>("detector", (e) => cb(e.payload)),
  onWizard: (cb: (state: WizardState) => void): Promise<UnlistenFn> =>
    listen<WizardState>("wizard", (e) => cb(e.payload)),
};
