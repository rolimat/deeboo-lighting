import { useEffect, useState } from "preact/hooks";
import { api, type WizardState } from "../api";
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
      class="rounded-card border border-border px-4 py-2"
    >
      Cancel
    </button>
  );
}

function Step({ state }: { state: WizardState }) {
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
        <StepIdentify
          segments={state.segments}
          onPiece={(n) => void api.stripIdentify(n).catch(swallow)}
          onSave={() => void api.wizardSave().catch(swallow)}
        />
      );
    case "saved":
      return <p>{`Connected: ${state.device.name} (${state.segments} segments).`}</p>;
    case "failed":
      return (
        <div class="space-y-3">
          <p role="alert" class="text-red">
            {state.message}
          </p>
          <button
            type="button"
            onClick={() => void api.wizardRetry().catch(swallow)}
            class="rounded-card bg-green px-4 py-2"
          >
            Retry
          </button>
        </div>
      );
  }
}

export function Wizard() {
  const [state, setState] = useState<WizardState>({ kind: "idle" });
  useEffect(() => {
    let off = () => {};
    let gone = false;
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
  return <Step state={state} />;
}
