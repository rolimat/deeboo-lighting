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

/** The editor's only door to Rust; components never import `@tauri-apps/api`. */
export const api = {
  getStatus: async (): Promise<StripStatus> => (await invoke<Status>("get_status")).strip,
  wizardStart: (userCode: string): Promise<void> => invoke("wizard_start", { userCode }),
  wizardChoose: (deviceId: string): Promise<void> => invoke("wizard_choose", { deviceId }),
  wizardRetry: (): Promise<void> => invoke("wizard_retry"),
  wizardCancel: (): Promise<void> => invoke("wizard_cancel"),
  stripIdentify: (segment: number): Promise<void> => invoke("strip_identify", { segment }),
  wizardSave: (): Promise<void> => invoke("wizard_save"),
  onWizard: (cb: (state: WizardState) => void): Promise<UnlistenFn> =>
    listen<WizardState>("wizard", (e) => cb(e.payload)),
};
