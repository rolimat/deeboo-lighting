import { type Status } from "./api";
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

export function mountPopover(root: HTMLElement): { update: (status: Status) => void } {
  const strip = el("div", "strip-status", stripText({ kind: "notConfigured" }));
  const card = el("div", "card");
  card.append(strip);
  root.replaceChildren(el("h1", "header", "Deeboo Lighting"), card);
  return {
    update(status) {
      const text = stripText(status.strip);
      if (strip.textContent !== text) strip.textContent = text;
    },
  };
}
