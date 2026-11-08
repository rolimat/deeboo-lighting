import { useEffect, useState } from "preact/hooks";
import type { StripStatus } from "../api";
import { api } from "./api";
import { ShapeEditor } from "./strip/ShapeEditor";
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
  useEffect(() => {
    api.getStatus().then(setStrip, () => setStrip({ kind: "error" }));
  }, []);
  return (
    <div class="flex h-screen">
      <nav class="w-40 border-r border-border p-3" aria-label="Sections">
        <a href="#strip" aria-current="page" class="block rounded-card bg-card px-3 py-2">
          Strip
        </a>
      </nav>
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
    </div>
  );
}
