import { api, type Status } from "./api";
import { el } from "./dom";

function stripText(strip: Status["strip"]): string {
  switch (strip.kind) {
    case "notConfigured":
      return "Strip · not connected";
    case "connecting":
      return "Strip · connecting…";
    case "connected":
      return `Strip · ${strip.name} (${strip.segments} segments)`;
    case "error":
      return "Strip · connection error";
  }
}

/** Debug-only probes for the sandbox check: mic level. */
function mountDevProbes(): HTMLElement {
  const box = el("div", "dev-probes");
  const micButton = el("button", "mic-level", "Mic level");
  const micResult = el("div", "mic-result");
  micButton.addEventListener("click", () => {
    api.devMicLevel().then(
      (db) => (micResult.textContent = `Mic · ${db.toFixed(1)} dBFS`),
      () => (micResult.textContent = "Mic · error"),
    );
  });
  box.append(micButton, micResult);
  return box;
}

export function mountPopover(root: HTMLElement): { update: (status: Status) => void } {
  const strip = el("div", "strip-status", stripText({ kind: "notConfigured" }));
  const card = el("div", "card");
  card.append(strip);
  let testButton: HTMLButtonElement | null = null;
  let probes: HTMLElement | null = null;
  const clock = el("div", "midi-clock");
  root.replaceChildren(el("h1", "header", "Deeboo Lighting"), card);
  return {
    update(status) {
      const text = stripText(status.strip);
      if (strip.textContent !== text) strip.textContent = text;
      if (status.debug && !probes) {
        probes = mountDevProbes();
        card.append(probes);
      } else if (!status.debug && probes) {
        probes.remove();
        probes = null;
      }
      if (status.clockTicks === null) clock.remove();
      else {
        clock.textContent = `MIDI clock · ${status.clockTicks} ticks`;
        if (!clock.isConnected) card.append(clock);
      }
      const wantTest = status.debug && status.strip.kind === "connected";
      if (wantTest && !testButton) {
        testButton = el("button", "test-strip", "Test strip");
        testButton.addEventListener("click", () => void api.devPaintTest());
        card.append(testButton);
      } else if (!wantTest && testButton) {
        testButton.remove();
        testButton = null;
      }
    },
  };
}
