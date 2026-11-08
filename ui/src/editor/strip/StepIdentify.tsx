export function StepIdentify({
  segments,
  onPiece,
  onSave,
}: {
  segments: number;
  onPiece: (segment: number) => void;
  onSave: () => void;
}) {
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
      <button type="button" onClick={onSave} class="rounded-card bg-green px-4 py-2">
        Save
      </button>
    </div>
  );
}
