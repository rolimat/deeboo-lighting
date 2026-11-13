import { help } from "./Help";
import { H } from "../help-texts";

export function ConfirmDelete({
  label,
  onConfirm,
  onCancel,
}: {
  label: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div class="fixed inset-0 z-10 grid place-items-center bg-black/50">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Confirm delete"
        class="rounded-card border border-border bg-card p-5"
      >
        <p class="mb-4">{`Delete "${label}"? You can undo this with ⌘Z.`}</p>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            {...help(H.cancelDelete)}
            class="rounded-card border border-border px-4 py-2"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onConfirm}
            {...help(H.confirmDelete)}
            class="rounded-card bg-red px-4 py-2 text-white"
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}
