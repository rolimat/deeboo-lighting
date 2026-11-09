import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type StripStatus =
  | { kind: "notConfigured" }
  | { kind: "connecting" }
  | { kind: "connected"; name: string; segments: number }
  | { kind: "error" }
  | { kind: "keyInvalid" }
  | { kind: "notFound" };

export interface Hsv {
  /** 0 to 360. */
  h: number;
  /** 0 to 1000. */
  s: number;
  /** 0 to 1000. */
  v: number;
}

/** The active set and the palette in force (Principal, Contraste, Suave). */
export interface Selection {
  set: string;
  /** Nothing pinned: the palette follows the set's rotation. */
  paletteAuto: boolean;
  palette: { name: string; colours: [Hsv, Hsv, Hsv] };
}

export interface Status {
  strip: StripStatus;
  bpm: number | null;
  playing: boolean;
  beatInBar: number | null;
  barInPhrase: number | null;
  phraseBars: number;
  moment: MomentName;
  auto: boolean;
  predicted: boolean;
  lightsOn: boolean;
  midi: { source: string | null; ok: boolean };
  audio: { ok: boolean; device?: string; levelDb?: number };
  selection: Selection;
}

export interface Settings {
  midiSource: string | null;
  audioInput: string | null;
  /** 4, 8 or 16. */
  phraseBars: number;
  /** 1 to 10. */
  sensitivity: number;
  /** 10 to 100. */
  brightnessPct: number;
  /** 0 to 500. */
  leadMs: number;
  lightsOn: boolean;
  /** Detection panel (editor): 8 to 30, 6 to 24, 3 to 15 dB. */
  calmKickDb: number;
  calmHighDb: number;
  riserDb: number;
  /** Show the `?` help tooltips in the editor. */
  helpTooltips: boolean;
}

export type MomentName = "rhythm" | "calm" | "buildUp";

export type UiCommand =
  { kind: "lights"; on: boolean } | { kind: "force"; moment: MomentName } | { kind: "auto" };

export const api = {
  getStatus: (): Promise<Status> => invoke("get_status"),
  getSettings: (): Promise<Settings> => invoke("get_settings"),
  saveSettings: (settings: Settings): Promise<Settings> => invoke("save_settings", { settings }),
  listMidiSources: (): Promise<string[]> => invoke("list_midi_sources"),
  listAudioInputs: (): Promise<string[]> => invoke("list_audio_inputs"),
  uiCommand: (cmd: UiCommand): Promise<void> => invoke("ui_command", { cmd }),
  /** Steps the active set by -1 or 1 (wraps); resolves to the new selection. */
  selectSet: (delta: -1 | 1): Promise<Selection> => invoke("select_set", { delta }),
  selectPalette: (delta: -1 | 1): Promise<Selection> => invoke("select_palette", { delta }),
  /** Makes a set live (Sets button); resolves to the new selection. */
  setLiveSet: (id: number): Promise<Selection> => invoke("set_live_set", { id }),
  quit: (): Promise<void> => invoke("quit"),
  /** Opens (or focuses) the editor window on a section. */
  openEditor: (section: "strip"): Promise<void> => invoke("open_editor", { section }),
  setWindowHeight: (height: number): Promise<void> =>
    getCurrentWindow().setSize(new LogicalSize(340, height)),
  /** The backend pushes the whole status on every change, only while the popover is visible (event `status`). */
  onStatus: (cb: (status: Status) => void): Promise<UnlistenFn> =>
    listen<Status>("status", (e) => cb(e.payload)),
};
