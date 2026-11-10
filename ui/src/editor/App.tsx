import { useEffect, useState } from "preact/hooks";
import type { StripStatus } from "../api";
import { api, type WizardState } from "./api";
import { PalettesTab } from "./palettes/PalettesTab";
import { SetsTab } from "./sets/SetsTab";
import { SequencesTab, type BarActions, type Sel } from "./sequences/SequencesTab";
import { StripTab } from "./strip/StripTab";
import { DetectionTab } from "./detection/DetectionTab";
import { Wizard, wizardLine } from "./strip/Wizard";
import { SetupTab } from "./setup/SetupTab";
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
const NO_UNDO_SETUP = "Settings apply at once; use Reset to defaults instead.";

export function App() {
  const [strip, setStrip] = useState<StripStatus | null>(null);
  const [repair, setRepair] = useState(false);
  const [wizard, setWizard] = useState<WizardState>({ kind: "idle" });
  const refresh = () => api.getStatus().then(setStrip, () => setStrip({ kind: "error" }));
  const [tab, setTab] = useState<Tab>(initialTab);
  const store = useLibrary();
  const [actions, setActions] = useState<BarActions | null>(null);
  const [openSeq, setOpenSeq] = useState<Sel | null>(null);
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
    void refresh();
  }, [tab]); // current after a re-pair: the tab was left and re-entered
  const label = TABS.find(([id]) => id === tab)?.[1] ?? "";
  // Until each tab uses the shared store, undo/redo are only live on tabs that edit the library.
  const showStrip = strip?.kind === "connected" && !repair;
  const libraryTab =
    tab === "sets" || tab === "sequences" || tab === "palettes" || (tab === "strip" && showStrip);
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
          {...(tab === "sets" || tab === "sequences" || tab === "palettes" ? actions : null)}
          save={store.save}
          undoDisabledHint={libraryTab ? undefined : tab === "setup" ? NO_UNDO_SETUP : NO_UNDO}
        />
        <section class="min-h-0 flex-1 overflow-auto p-6" aria-label={label}>
          {tab === "sets" && (
            <SetsTab
              store={store}
              onActions={setActions}
              onOpenSequence={(s) => {
                setOpenSeq(s);
                setTab("sequences");
              }}
            />
          )}
          {tab === "sequences" && (
            <SequencesTab
              store={store}
              onActions={setActions}
              initial={openSeq}
              stripSegments={strip?.kind === "connected" ? strip.segments : undefined}
            />
          )}
          {tab === "palettes" && (
            <PalettesTab store={store} onActions={setActions} onGoSets={() => setTab("sets")} />
          )}
          {tab === "detection" && <DetectionTab onGoSetup={() => setTab("setup")} />}
          {tab === "setup" && <SetupTab />}
          {tab === "strip" &&
            (strip?.kind === "connected" && !repair ? (
              <StripTab
                store={store}
                strip={strip}
                onRepair={() => setRepair(true)}
                onWired={() => void refresh()}
              />
            ) : (
              <>
                <p class="mb-4 text-dim">{wizardLine(wizard) ?? describe(strip)}</p>
                <Wizard
                  onState={setWizard}
                  onDone={() => {
                    setRepair(false);
                    setWizard({ kind: "idle" });
                    void refresh();
                    // The wizard rebuilt the library for the new strip.
                    void api.getLibrary().then(store.reset, () => {});
                  }}
                />
                {repair && strip?.kind === "connected" && wizard.kind === "idle" && (
                  <button
                    type="button"
                    onClick={() => setRepair(false)}
                    class="mt-4 rounded-card border border-border px-4 py-2"
                  >
                    Back to the strip
                  </button>
                )}
              </>
            ))}
        </section>
      </div>
    </HelpProvider>
  );
}
