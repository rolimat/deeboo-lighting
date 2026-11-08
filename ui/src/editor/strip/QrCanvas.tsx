import { useEffect, useRef } from "preact/hooks";
import type { QrMatrix } from "../api";

const SCALE = 6;
const QUIET = 4;
// A QR code needs real black on white to scan; these are not UI colours.
const DARK = "#000000";
const LIGHT = "#ffffff";

/** Draws the module matrix locally: no SVG, no data: URLs, nothing leaves the webview. */
export function QrCanvas({ qr }: { qr: QrMatrix }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const px = (qr.size + QUIET * 2) * SCALE;
  useEffect(() => {
    const ctx = ref.current?.getContext("2d");
    if (!ctx) return;
    ctx.fillStyle = LIGHT;
    ctx.fillRect(0, 0, px, px);
    ctx.fillStyle = DARK;
    qr.modules.forEach((dark, i) => {
      if (!dark) return;
      const x = (i % qr.size) + QUIET;
      const y = Math.floor(i / qr.size) + QUIET;
      ctx.fillRect(x * SCALE, y * SCALE, SCALE, SCALE);
    });
  }, [qr, px]);
  return <canvas ref={ref} width={px} height={px} role="img" aria-label="QR code" />;
}
