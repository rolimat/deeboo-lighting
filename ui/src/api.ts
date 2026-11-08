import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type StripStatus =
  | { kind: "notConfigured" }
  | { kind: "connecting" }
  | { kind: "connected"; name: string; segments: number }
  | { kind: "error" };

export interface Status {
  strip: StripStatus;
  /** True in debug builds only. */
  debug: boolean;
  /** MIDI clock ticks seen by the debug probe; null in release. */
  clockTicks: number | null;
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
  /** The backend pushes the whole status on every change (event `status`). */
  onStatus: (cb: (status: Status) => void): Promise<UnlistenFn> =>
    listen<Status>("status", (e) => cb(e.payload)),
  /** Debug builds only: the backend rejects it in release. */
  devPaintTest: (): Promise<void> => invoke("dev_paint_test"),
  /** Debug builds only: peak of the default input over 1 s, in dBFS. */
  devMicLevel: (): Promise<number> => invoke("dev_mic_level"),
};
