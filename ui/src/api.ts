import { invoke } from "@tauri-apps/api/core";

export type StripStatus =
  | { kind: "notConfigured" }
  | { kind: "connecting" }
  | { kind: "connected"; name: string; segments: number }
  | { kind: "error" };

export interface Status {
  strip: StripStatus;
}

export const api = {
  getStatus: (): Promise<Status> => invoke("get_status"),
};
