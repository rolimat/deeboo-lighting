import { api, type Status } from "./api";

/**
 * Keeps the view in step with the backend: listens first, then reads, so no change is missed.
 * The popover also tells the backend whether it is on screen (the engine publishes only then)
 * and re-reads the status each time it becomes visible, so it never opens stale.
 */
export async function syncStatus(update: (status: Status) => void): Promise<void> {
  let pushed = false;
  await api.onStatus((status) => {
    pushed = true;
    update(status);
  });
  const initial = await api.getStatus();
  // A push that landed while the read was in flight is newer than the read.
  if (!pushed) update(initial);

  const report = () => {
    const visible = document.visibilityState === "visible";
    // Best effort: the tray reports the same flag.
    void api.popoverVisible(visible).catch(() => {});
    if (visible) void api.getStatus().then(update, () => {});
  };
  document.addEventListener("visibilitychange", report);
  report();
}
