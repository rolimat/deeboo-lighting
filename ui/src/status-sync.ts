import { api, type Status } from "./api";

/** Keeps the view in step with the backend: listens first, then reads, so no change is missed. */
export async function syncStatus(update: (status: Status) => void): Promise<void> {
  let pushed = false;
  await api.onStatus((status) => {
    pushed = true;
    update(status);
  });
  const initial = await api.getStatus();
  // A push that landed while the read was in flight is newer than the read.
  if (!pushed) update(initial);
}
