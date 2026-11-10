import { useRef } from "preact/hooks";
import type { Hsv } from "../api";
import { css } from "../sequences/RolePicker";
import { fromWheel, wheelPoint } from "./colour";

const KEY_HUE = 5;
const KEY_SAT = 25;

/** Hue (angle) and saturation (radius) picker; the brightness is not touched. */
export function ColourWheel({ value, onChange }: { value: Hsv; onChange: (c: Hsv) => void }) {
  const el = useRef<HTMLDivElement>(null);
  const down = useRef(false);
  const at = (e: MouseEvent) => {
    const r = el.current?.getBoundingClientRect();
    if (!r || !r.width) return;
    onChange(
      fromWheel(
        ((e.clientX - r.left) / r.width) * 2 - 1,
        ((e.clientY - r.top) / r.height) * 2 - 1,
        value.v,
      ),
    );
  };
  const p = wheelPoint(value);
  return (
    <div
      ref={el}
      data-wheel
      role="group"
      tabIndex={0}
      aria-label="Colour wheel: hue and saturation"
      class="wheel relative h-44 w-44 shrink-0 touch-none rounded-full"
      onPointerDown={(e) => {
        down.current = true;
        (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
        at(e);
      }}
      onPointerMove={(e) => down.current && at(e)}
      onPointerUp={() => (down.current = false)}
      onPointerCancel={() => (down.current = false)}
      onKeyDown={(e) => {
        const dh = e.key === "ArrowRight" ? KEY_HUE : e.key === "ArrowLeft" ? -KEY_HUE : 0;
        const ds = e.key === "ArrowUp" ? KEY_SAT : e.key === "ArrowDown" ? -KEY_SAT : 0;
        if (!dh && !ds) return;
        e.preventDefault();
        onChange({
          ...value,
          h: (value.h + dh + 360) % 360,
          s: Math.max(0, Math.min(1000, value.s + ds)),
        });
      }}
    >
      <span
        aria-hidden="true"
        class="pointer-events-none absolute h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_#000]"
        style={{
          left: `${50 + p.x * 50}%`,
          top: `${50 + p.y * 50}%`,
          background: css(value),
        }}
      />
    </div>
  );
}
