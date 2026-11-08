import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { StripStatus, Status } from "../api";

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

export type Interval = { beats: number } | { bars: number } | { phrases: number };
export type SeqRef = { id: number } | "generated";
export interface Piece {
  x: number;
  y: number;
  angle: 0 | 90 | 180 | 270;
  len: 1 | 2 | 3 | 4;
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
  palettes: number[];
  order: "inOrder" | "random" | "fixed";
  interval: Interval;
}
export interface LibrarySet {
  id: number;
  name: string;
  rhythm: MomentSet;
  calm: MomentSet;
  buildup: MomentSet;
}
/**
 * Mirrors `light_core::library::Library`. Ids are assigned by Rust: an id the saved library does not
 * have is a client-side handle for a new entry (use any unused number, e.g. a counter from 1_000_000);
 * Rust gives it the next free id and rewrites every reference to it. `saveLibrary` returns the
 * library with the real ids.
 */
export interface Library {
  schemaVersion: 1;
  shape: { pieces: Piece[] };
  palettes: [number, Palette][];
  sequences: [number, Sequence][];
  sets: LibrarySet[];
  activeSet: number;
  activePalette: number | null;
}

/** The editor's only door to Rust; components never import `@tauri-apps/api`. */
export const api = {
  getStatus: async (): Promise<StripStatus> => (await invoke<Status>("get_status")).strip,
  getLibrary: (): Promise<Library> => invoke("get_library"),
  /** Rejects with a short message; the previous library is kept. */
  saveLibrary: (library: Library): Promise<Library> => invoke("save_library", { library }),
  wizardStart: (userCode: string): Promise<void> => invoke("wizard_start", { userCode }),
  wizardChoose: (deviceId: string): Promise<void> => invoke("wizard_choose", { deviceId }),
  wizardRetry: (): Promise<void> => invoke("wizard_retry"),
  wizardCancel: (): Promise<void> => invoke("wizard_cancel"),
  stripIdentify: (segment: number): Promise<void> => invoke("strip_identify", { segment }),
  /** Flashes a segment (1-based) through the app's own strip connection; rejects with a short message. */
  identifySegment: (segment: number): Promise<void> => invoke("identify_segment", { segment }),
  wizardSave: (): Promise<void> => invoke("wizard_save"),
  onWizard: (cb: (state: WizardState) => void): Promise<UnlistenFn> =>
    listen<WizardState>("wizard", (e) => cb(e.payload)),
};
