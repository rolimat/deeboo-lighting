import { useEffect, useState } from "preact/hooks";
import { api, type WizardState } from "../api";
import { help } from "../shell/Help";
import { H } from "../help-texts";
import { QrCanvas } from "./QrCanvas";
import { StepChoose } from "./StepChoose";
import { StepIdentify } from "./StepIdentify";
import { StepSearch } from "./StepSearch";
import { StepSegments } from "./StepSegments";
import { StepUserCode } from "./StepUserCode";

/** Fire and forget: a rejected command must not crash the view; Rust re-emits the real state. */
const swallow = () => {};

function Countdown({ expiresAtUs }: { expiresAtUs: number }) {
  const left = () => Math.max(0, Math.round((expiresAtUs / 1000 - Date.now()) / 1000));
  const [secs, setSecs] = useState(left);
  // A visible countdown is a user-facing wait, not an engine poll (plan decision 4).
  useEffect(() => {
    const t = setInterval(() => setSecs(left()), 1000);
    return () => clearInterval(t);
  }, [expiresAtUs]);
  return (
    <p class="text-dim">
      {Math.floor(secs / 60)}:{String(secs % 60).padStart(2, "0")}
    </p>
  );
}

function CancelButton() {
  return (
    <button
      type="button"
      onClick={() => void api.wizardCancel().catch(swallow)}
      {...help(H.wizardCancel)}
      class="rounded-card border border-border px-4 py-2"
    >
      Cancel
    </button>
  );
}

/** What the Strip header says while the wizard runs; `null` when it has nothing to add. */
export function wizardLine(state: WizardState): string | null {
  switch (state.kind) {
    case "waitingScan":
      return "Waiting for you to confirm in Smart Life";
    case "chooseStrip":
      return "Choose your strip";
    case "searching":
      return `Looking for ${state.device.name} on your wifi`;
    case "listening":
      return `Connected to ${state.device.name} — waiting for paint data`;
    case "identify":
      return `Connected to ${state.device.name} — checking segments`;
    case "saved":
      return `Connected to ${state.device.name}`;
    default:
      return null;
  }
}

function Step({ state, onDone }: { state: WizardState; onDone: () => void }) {
  switch (state.kind) {
    case "idle":
      return <StepUserCode onSubmit={api.wizardStart} />;
    case "waitingScan":
      return (
        <div class="space-y-3">
          <QrCanvas qr={state.qr} />
          <p>Scan this code with Smart Life and confirm.</p>
          <Countdown expiresAtUs={state.expiresAtUs} />
          <CancelButton />
        </div>
      );
    case "chooseStrip":
      return (
        <div class="space-y-3">
          <StepChoose
            devices={state.devices}
            onChoose={(id) => void api.wizardChoose(id).catch(swallow)}
          />
          <CancelButton />
        </div>
      );
    case "searching":
      return (
        <div class="space-y-3">
          <StepSearch device={state.device} help={state.help} />
          <CancelButton />
        </div>
      );
    case "listening":
      return (
        <div class="space-y-3">
          <StepSegments />
          <CancelButton />
        </div>
      );
    case "identify":
      return (
        <div class="space-y-3">
          <StepIdentify
            detected={state.segments}
            onPiece={(n) => void api.stripIdentify(n).catch(swallow)}
            onSave={(wired) => api.wizardSave(wired)}
          />
          <CancelButton />
        </div>
      );
    case "saved":
      return (
        <div class="space-y-4">
          <p>{`Connected: ${state.device.name} (${state.segments} segments).`}</p>
          <button
            type="button"
            onClick={onDone}
            class="rounded-card bg-green px-4 py-2"
            {...help(H.drawShape)}
          >
            Draw the shape
          </button>
        </div>
      );
    case "failed":
      return (
        <div class="space-y-3">
          <p role="alert" class="text-red">
            {state.message}
          </p>
          <div class="flex gap-2">
            <button
              type="button"
              onClick={() => void api.wizardRetry().catch(swallow)}
              {...help(H.wizardRetry)}
              class="rounded-card bg-green px-4 py-2"
            >
              Retry
            </button>
            <CancelButton />
          </div>
        </div>
      );
  }
}

export function Wizard({
  onState,
  onDone,
}: {
  /** Every wizard state, so the header can show it instead of the runtime's. */
  onState?: (s: WizardState) => void;
  /** After the strip is saved: leave the wizard for the shape. */
  onDone?: () => void;
}) {
  const [state, setState] = useState<WizardState>({ kind: "idle" });
  useEffect(() => onState?.(state), [state]);
  useEffect(() => {
    let off = () => {};
    let gone = false;
    // A remount (tab switch) while the run continues in Rust: ask where it is. An event that
    // arrived first is newer and wins.
    void api
      .wizardState()
      .then((s) => !gone && setState((cur) => (cur.kind === "idle" ? s : cur)))
      .catch(swallow);
    void api.onWizard(setState).then((un) => {
      if (gone) un();
      else off = un;
    });
    // Closing the window mid-login must revoke the cloud session.
    const cancel = () => void api.wizardCancel().catch(swallow);
    window.addEventListener("beforeunload", cancel);
    return () => {
      gone = true;
      off();
      window.removeEventListener("beforeunload", cancel);
    };
  }, []);
  return <Step state={state} onDone={onDone ?? swallow} />;
}
