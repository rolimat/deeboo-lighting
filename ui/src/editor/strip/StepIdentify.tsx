import { useState } from "preact/hooks";

export function StepIdentify({
  segments,
  onPiece,
  onSave,
}: {
  segments: number;
  onPiece: (segment: number) => void;
  /** Rejects with the text to show when the strip could not be saved or handed to the engine. */
  onSave: () => Promise<void>;
}) {
  const [error, setError] = useState("");
  const save = () => {
    setError("");
    onSave().catch((e: unknown) => setError(String(e)));
  };
  return (
    <div class="space-y-4">
      <div class="flex gap-1">
        {Array.from({ length: segments }, (_, i) => i + 1).map((n) => (
          <button
            key={n}
            type="button"
            data-piece
            aria-label={`Segment ${n}`}
            onClick={() => onPiece(n)}
            class="h-10 min-w-0 flex-1 rounded-card bg-card"
          >
            {n}
          </button>
        ))}
      </div>
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
