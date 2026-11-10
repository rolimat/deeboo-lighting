import { useEffect, useRef } from "preact/hooks";
import { useHelp } from "../shell/Help";

interface Props {
  name: string;
  /** The words on the label, e.g. "Calm when 14 dB below". */
  words: string;
  value: number;
  min: number;
  max: number;
  /** Position inside the lane, 0 (top) to 100. */
  topPct: number;
  /** dB margin the pointer height means, before clamping. */
  toValue: (clientY: number) => number;
  onChange: (value: number) => void;
  hint: string;
}

/** A dashed line over a lane: drag it, or focus it and use the arrow keys (1 dB per press). */
export function ThresholdLine(p: Props) {
  const stop = useRef<(() => void) | null>(null);
  useEffect(() => () => stop.current?.(), []);
  const set = (v: number) => p.onChange(Math.min(p.max, Math.max(p.min, Math.round(v))));
  const down = (e: Event) => {
    e.preventDefault();
    const move = (m: MouseEvent) => set(p.toValue(m.clientY));
    const end = () => stop.current?.();
    stop.current = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      stop.current = null;
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
  };
  const key = (e: KeyboardEvent) => {
    const d =
      e.key === "ArrowUp" || e.key === "ArrowRight"
        ? 1
        : e.key === "ArrowDown" || e.key === "ArrowLeft"
          ? -1
          : 0;
    if (!d) return;
    e.preventDefault();
    set(p.value + d);
  };
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={p.name}
      aria-valuemin={p.min}
      aria-valuemax={p.max}
      aria-valuenow={p.value}
      aria-valuetext={p.words}
      onPointerDown={down}
      onKeyDown={key}
      style={{ top: `${p.topPct}%` }}
      class="group absolute inset-x-0 z-10 -mt-2 h-4 cursor-ns-resize outline-none"
      {...useHelp(p.hint)}
    >
      <div class="absolute inset-x-0 top-2 border-t-2 border-dashed border-accent group-focus-visible:border-solid" />
      <span class="absolute right-1.5 -top-1 select-none rounded border border-accent bg-card px-1.5 text-[10px] text-accent">
        {p.words}
      </span>
    </div>
  );
}
