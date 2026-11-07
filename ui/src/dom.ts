export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  text = "",
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

/** Button whose glyph sits in a `.glyph` span (pointer-events none) so the button is always the event target. */
export function glyphButton(className: string, glyph: string): HTMLButtonElement {
  const b = el("button", className);
  b.append(el("span", "glyph", glyph));
  return b;
}
