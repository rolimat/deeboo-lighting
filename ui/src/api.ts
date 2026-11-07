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
}

export const api = {
  getStatus: (): Promise<Status> => invoke("get_status"),
  /** The backend pushes the whole status on every change (event `status`). */
  onStatus: (cb: (status: Status) => void): Promise<UnlistenFn> =>
    listen<Status>("status", (e) => cb(e.payload)),
  /** Debug builds only: the backend rejects it in release. */
  devPaintTest: (): Promise<void> => invoke("dev_paint_test"),
};
