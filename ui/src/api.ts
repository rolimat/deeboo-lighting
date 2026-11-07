import { invoke } from "@tauri-apps/api/core";

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
  /** Debug builds only: the backend rejects it in release. */
  devPaintTest: (): Promise<void> => invoke("dev_paint_test"),
};
