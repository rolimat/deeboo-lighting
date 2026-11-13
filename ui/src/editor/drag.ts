/** Internal drags carry only this type, so no text input can ever receive their payload. */
export const DRAG_TYPE = "application/x-deeboo";

export const COLOUR_TYPE = "application/x-deeboo-colour";
export const ROLE_TYPE = "application/x-deeboo-role";

/** Cancels every drag or drop no internal drop zone took, so a dropped file or link never loads. */
export function installDropGuard(): () => void {
  const over = (e: DragEvent) => {
    if (e.defaultPrevented) return; // an internal drop zone took it
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "none";
  };
  const drop = (e: DragEvent) => e.preventDefault();
  document.addEventListener("dragover", over);
  document.addEventListener("drop", drop);
  return () => {
    document.removeEventListener("dragover", over);
    document.removeEventListener("drop", drop);
  };
}
