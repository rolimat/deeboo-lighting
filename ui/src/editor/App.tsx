import { useEffect, useState } from "preact/hooks";
import type { StripStatus } from "../api";
import { api } from "./api";
import { PaletteEditor } from "./palettes/PaletteEditor";
import { SetEditor } from "./sets/SetEditor";
import { SequenceList } from "./sequences/SequenceList";
import { ShapeEditor } from "./strip/ShapeEditor";
import { DetectionPanel } from "./detection/DetectionPanel";
import { Wizard } from "./strip/Wizard";

function describe(s: StripStatus | null): string {
  switch (s?.kind) {
    case "connected":
      return `${s.name} (${s.segments} segments)`;
    case "connecting":
      return "connecting…";
    case "error":
      return "connection error";
    default:
      return "not connected";
  }
}

export function App() {
  const [strip, setStrip] = useState<StripStatus | null>(null);
  const [repair, setRepair] = useState(false);
  const [section, setSection] = useState<"Strip" | "Sequences" | "Palettes" | "Sets" | "Detection">(
    "Strip",
  );
  useEffect(() => {
    api.getStatus().then(setStrip, () => setStrip({ kind: "error" }));
  }, []);
  return (
    <div class="flex h-screen">
      <nav class="w-40 space-y-1 border-r border-border p-3" aria-label="Sections">
        {(["Strip", "Sequences", "Palettes", "Sets", "Detection"] as const).map((name) => (
          <button
            type="button"
            key={name}
            aria-current={name === section ? "page" : undefined}
            onClick={() => setSection(name)}
            class={`block w-full rounded-card px-3 py-2 text-left ${name === section ? "bg-card" : ""}`}
          >
            {name}
          </button>
        ))}
      </nav>
      {section === "Sequences" && (
        <section class="min-w-0 flex-1 overflow-auto p-6" aria-label="Sequences">
          <h1 class="mb-4 text-lg">Sequences</h1>
          <SequenceList />
        </section>
      )}
      {section === "Palettes" && (
        <section class="min-w-0 flex-1 overflow-auto p-6" aria-label="Palettes">
          <h1 class="mb-4 text-lg">Palettes</h1>
          <PaletteEditor />
        </section>
      )}
      {section === "Sets" && (
        <section class="min-w-0 flex-1 overflow-auto p-6" aria-label="Sets">
          <h1 class="mb-4 text-lg">Sets</h1>
          <SetEditor />
        </section>
      )}
      {section === "Detection" && (
        <section class="min-w-0 flex-1 overflow-auto p-6" aria-label="Detection">
          <h1 class="mb-4 text-lg">Detection</h1>
          <DetectionPanel />
        </section>
      )}
      {section === "Strip" && (
        <section class="flex-1 p-6" aria-label="Strip">
          <h1 class="text-lg">Strip</h1>
          <p class="mb-4 text-dim">{describe(strip)}</p>
          {strip?.kind === "connected" && !repair ? (
            <div class="space-y-4">
              <ShapeEditor segments={strip.segments} />
              <button
                type="button"
                onClick={() => setRepair(true)}
                class="rounded-card border border-border px-4 py-2"
              >
                Pair a different strip
              </button>
            </div>
          ) : (
            <Wizard />
          )}
        </section>
      )}
    </div>
  );
}
