import { useState } from "preact/hooks";

export function StepIdentify({
  detected,
  onPiece,
  onSave,
}: {
  /** What the controller reports. */
  detected: number;
  onPiece: (segment: number) => void;
  /** Rejects with the text to show when the strip could not be saved or handed to the engine. */
  onSave: (wired: number) => Promise<void>;
}) {
  const [error, setError] = useState("");
  const [wired, setWired] = useState(detected);
  const clamp = (v: number) =>
    Number.isFinite(v) ? Math.min(detected, Math.max(1, Math.round(v))) : detected;
  const save = () => {
    setError("");
    onSave(wired).catch((e: unknown) => setError(String(e)));
  };
  return (
    <div class="space-y-4">
      <p>Click a number to light that segment on the strip.</p>
      <div class="flex gap-1">
        {Array.from({ length: detected }, (_, i) => i + 1).map((n) => (
          <button
            key={n}
            type="button"
            data-piece
            data-segment-number
            data-wired={String(n <= wired)}
            aria-label={`Segment ${n}`}
            onClick={() => onPiece(n)}
            class={`h-10 min-w-0 flex-1 rounded-card bg-card ${n <= wired ? "" : "border border-dashed border-border opacity-40"}`}
          >
            {n}
          </button>
        ))}
      </div>
      <label class="flex items-center gap-2">
        <span>{`Detected ${detected}. How many light up?`}</span>
        <input
          type="number"
          min={1}
          max={detected}
          value={wired}
          aria-label="Wired segments"
          onInput={(e) => setWired(clamp(e.currentTarget.valueAsNumber))}
          class="w-16 rounded-lg border border-border bg-bg px-2 py-1"
        />
      </label>
      {error && (
        <p role="alert" class="text-red">
          {error}
        </p>
      )}
      <button type="button" onClick={save} class="rounded-card bg-green px-4 py-2">
        Save
      </button>
    </div>
  );
}
