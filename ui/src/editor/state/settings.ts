import type { Settings } from "../../api";
import { api } from "../api";

/** Saves only `patch` on top of the settings as they are now, so a tab never reverts changes made elsewhere. */
export const patchSettings = (patch: Partial<Settings>): Promise<Settings> =>
  api.getSettings().then((cur) => api.saveSettings({ ...cur, ...patch }));
