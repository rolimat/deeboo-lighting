import type { Hsv } from "../api";

/** The wheel's unit disc: hue is the angle clockwise from the east, saturation the distance from the centre. */
export function fromWheel(x: number, y: number, v: number): Hsv {
  const r = Math.min(1, Math.hypot(x, y));
  const deg = (Math.atan2(y, x) * 180) / Math.PI;
  return { h: Math.round((deg + 360) % 360) % 360, s: Math.round(r * 1000), v };
}

export function wheelPoint({ h, s }: Hsv): { x: number; y: number } {
  const a = (h * Math.PI) / 180;
  return { x: (s / 1000) * Math.cos(a), y: (s / 1000) * Math.sin(a) };
}

function rgb({ h, s, v }: Hsv): [number, number, number] {
  const sat = s / 1000;
  const val = v / 1000;
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round(255 * (val - val * sat * Math.max(0, Math.min(k, 4 - k, 1))));
  };
  return [f(5), f(3), f(1)];
}

export const toHex = (c: Hsv): string =>
  "#" +
  rgb(c)
    .map((x) => x.toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase();

/** `#RRGGBB` (the `#` is optional) to HSV in the library's units, or null when it is not a colour. */
export function fromHex(text: string): Hsv | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(text.trim());
  if (!m?.[1]) return null;
  const n = parseInt(m[1], 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((x) => x / 255) as [
    number,
    number,
    number,
  ];
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  let h = 0;
  if (d > 0) {
    if (max === r) h = ((g - b) / d + 6) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
  }
  return {
    h: Math.round(h * 60) % 360,
    s: max === 0 ? 0 : Math.round((d / max) * 1000),
    v: Math.round(max * 1000),
  };
}
