import { useEffect, useState } from "preact/hooks";
import type { StripStatus } from "../api";
import { api } from "./api";
import { PalettesTab } from "./palettes/PalettesTab";
import { SetEditor } from "./sets/SetEditor";
import { SequencesTab, type BarActions } from "./sequences/SequencesTab";
import { ShapeEditor } from "./strip/ShapeEditor";
import { DetectionPanel } from "./detection/DetectionPanel";
import { Wizard } from "./strip/Wizard";
import { Setup } from "./setup/Setup";
import { HelpProvider } from "./shell/Help";
import { TopBar, TABS, type Tab } from "./shell/TopBar";
import { useLibrary } from "./state/library";

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

/** `open_editor`'s section arrives as `?section=` on the window URL; Strip is the default. */
function initialTab(): Tab {
  const q = new URLSearchParams(location.search).get("section")?.toLowerCase();
  return TABS.find(([id]) => id === q)?.[0] ?? "strip";
}

const NO_UNDO = "This tab applies changes at once, so there is nothing to undo.";

export function App() {
  const [strip, setStrip] = useState<StripStatus | null>(null);
  const [repair, setRepair] = useState(false);
  const [tab, setTab] = useState<Tab>(initialTab);
  const store = useLibrary();
  const [actions, setActions] = useState<BarActions | null>(null);
  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    api
      .onSection((id) => {
        const t = TABS.find(([name]) => name === id)?.[0];
        if (t) setTab(t);
      })
      .then(
        (u) => (alive ? (off = u) : u()),
        () => {},
      );
    return () => {
      alive = false;
      off?.();
    };
  }, []);
  useEffect(() => {
    api.getStatus().then(setStrip, () => setStrip({ kind: "error" }));
  }, [tab]); // current after a re-pair: the tab was left and re-entered
  const label = TABS.find(([id]) => id === tab)?.[1] ?? "";
  // Until each tab uses the shared store, undo/redo are only live on tabs that edit the library.
  const libraryTab = tab === "sets" || tab === "sequences" || tab === "palettes";
  return (
    <HelpProvider>
      <div class="flex h-screen flex-col">
        <TopBar
          tab={tab}
          onTab={setTab}
          canUndo={store.canUndo}
          canRedo={store.canRedo}
          onUndo={store.undo}
          onRedo={store.redo}
          {...(tab === "sequences" || tab === "palettes" ? actions : null)}
          save={store.save}
          undoDisabledHint={libraryTab ? undefined : NO_UNDO}
        />
        <section class="min-h-0 flex-1 overflow-auto p-6" aria-label={label}>
          {tab === "sets" && <SetEditor />}
          {tab === "sequences" && (
            <SequencesTab
              store={store}
              onActions={setActions}
              stripSegments={strip?.kind === "connected" ? strip.segments : undefined}
            />
          )}
          {tab === "palettes" && (
            <PalettesTab store={store} onActions={setActions} onGoSets={() => setTab("sets")} />
          )}
          {tab === "detection" && <DetectionPanel />}
          {tab === "setup" && <Setup />}
          {tab === "strip" && (
            <>
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
            </>
          )}
        </section>
      </div>
    </HelpProvider>
  );
}
