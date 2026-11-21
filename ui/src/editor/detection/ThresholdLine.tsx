import { useEffect, useRef } from "preact/hooks";
import { H } from "../help-texts";
import { help } from "../shell/Help";

interface Props {
  name: string;
  /** The words on the label, e.g. "Calm when 14.0 dB below". */
  words: string;
  value: number;
  min: number;
  max: number;
  /** Position inside the lane in percent from the top; outside 0..100 means outside the view. */
  topPct: number;
  /** dB margin the pointer height means, before clamping. */
  toValue: (clientY: number) => number;
  onChange: (value: number) => void;
  /** Clicked on the pinned label of a line outside the view. */
  onOutside: () => void;
  /** The tooltip text. */
  hint: string;
  /** Where the line starts, in percent of the lane width. */
  leftPct?: number;
  /** True on pointer-down, false when the drag ends. */
  onDrag?: (active: boolean) => void;
}

/** A dashed line over a lane with its green label at the left end: drag either, or focus the line and use the arrow keys (1 dB per press, Shift 0.1 dB). */
export function ThresholdLine(p: Props) {
  const stop = useRef<((notify?: boolean) => void) | null>(null);
  useEffect(() => () => stop.current?.(false), []);
  const onDrag = useRef(p.onDrag);
  onDrag.current = p.onDrag;
  const set = (v: number, fine: boolean) =>
    p.onChange(Math.min(p.max, Math.max(p.min, fine ? Math.round(v * 10) / 10 : Math.round(v))));
  const down = (e: Event) => {
    e.preventDefault();
    const move = (m: MouseEvent) => set(p.toValue(m.clientY), m.shiftKey);
    const end = () => stop.current?.(true);
    stop.current = (notify = true) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      window.removeEventListener("blur", end);
      stop.current = null;
      if (notify) onDrag.current?.(false);
    };
    onDrag.current?.(true);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    window.addEventListener("blur", end);
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
    set(p.value + d * (e.shiftKey ? 0.1 : 1), true);
  };
  const left = p.leftPct ?? 0;
  const label =
    "absolute z-20 -translate-y-1/2 select-none whitespace-nowrap rounded border bg-card px-1.5 text-small text-accent";
  const above = p.topPct < 0;
  const out = above || p.topPct > 100;
  return (
    <>
      {out && (
        <button
          type="button"
          onClick={p.onOutside}
          style={{ top: above ? "9%" : "91%", left: "2.25rem" }}
          class={`${label} cursor-pointer border-dashed border-accent`}
          {...help(H.bandZoomOutside)}
        >
          {above ? "▲ " : "▼ "}
          {p.words}
        </button>
      )}
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
        style={{ top: `${Math.min(100, Math.max(0, p.topPct))}%`, left: `${left}%` }}
        class="group absolute right-0 z-10 -mt-2 h-4 cursor-ns-resize outline-none"
        {...help(`${p.hint} ${H.thresholdDrag}`)}
      >
        <div
          class={`absolute inset-x-0 top-2 border-t-2 border-dashed border-accent group-focus-visible:border-solid ${out ? "invisible" : ""}`}
        />
      </div>
      {!out && (
        <span
          data-label
          onPointerDown={down}
          style={{
            top: `${Math.min(91, Math.max(9, p.topPct))}%`,
            left: left ? `calc(${left}% + 0.5rem)` : "2.25rem",
          }}
          class={`${label} cursor-ns-resize border-accent`}
          {...help(`${p.hint} ${H.thresholdDrag}`)}
        >
          {p.words}
        </span>
      )}
    </>
  );
}
