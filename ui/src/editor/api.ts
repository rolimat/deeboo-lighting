import { invoke } from "@tauri-apps/api/core";
import type { StripStatus, Status } from "../api";

/** The editor's only door to Rust; components never import `@tauri-apps/api`. */
export const api = {
  getStatus: async (): Promise<StripStatus> => (await invoke<Status>("get_status")).strip,
};
