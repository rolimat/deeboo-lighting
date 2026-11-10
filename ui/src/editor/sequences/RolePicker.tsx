import type { Hsv, Palette } from "../api";

/** HSV (H 0-360, S/V 0-1000) to a CSS colour. */
export function css({ h, s, v }: Hsv): string {
  const sat = s / 1000;
  const val = v / 1000;
  const l = val * (1 - sat / 2);
  const sl = l === 0 || l === 1 ? 0 : (val - l) / Math.min(l, 1 - l);
  return `hsl(${h} ${Math.round(sl * 100)}% ${Math.round(l * 100)}%)`;
}

export function roleColour(role: string, p: Palette): string {
  switch (role) {
    case "principal":
    case "contraste":
    case "suave":
      return css(p[role]);
    case "white":
      return "#fff";
    default:
      return "var(--color-off)";
  }
}
