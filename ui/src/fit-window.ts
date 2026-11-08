import { api } from "./api";

/** Keeps the popover window exactly as tall as its content. */
export function fitWindow(node: HTMLElement): void {
  new ResizeObserver((entries) => {
    const h = entries[0]?.borderBoxSize[0]?.blockSize;
    if (h) void api.setWindowHeight(Math.ceil(h)).catch(() => {});
  }).observe(node);
}
