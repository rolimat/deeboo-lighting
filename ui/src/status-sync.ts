import { api, type Status } from "./api";

/**
 * Keeps the view in step with the backend: listens first, then reads, so no change is missed.
 * The popover also tells the backend whether it is on screen (the engine publishes only then)
 * and re-reads the status each time it becomes visible, so it never opens stale.
 */
export async function syncStatus(update: (status: Status) => void): Promise<void> {
  let pushes = 0;
  await api.onStatus((status) => {
    pushes++;
    update(status);
  });
  const initial = await api.getStatus();
  // A push that landed while the read was in flight is newer than the read.
  if (pushes === 0) update(initial);

  const report = () => {
    const visible = document.visibilityState === "visible";
    // Best effort: the tray reports the same flag.
    void api.popoverVisible(visible).catch(() => {});
    if (visible) {
      const before = pushes;
      // Like the first read: a push that lands while it is in flight is newer.
      void api.getStatus().then(
        (s) => pushes === before && update(s),
        () => {},
      );
    }
  };
  document.addEventListener("visibilitychange", report);
  report();
}
