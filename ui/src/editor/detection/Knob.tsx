import { useEffect, useRef } from "preact/hooks";
import { H } from "../help-texts";
import { help } from "../shell/Help";
import { fmtDb } from "./zoom";

interface Props {
  name: string;
  label: string;
  above: boolean;
  value: number;
  min: number;
  max: number;
  /** Double-click value. */
  def: number;
  hint: string;
  onChange: (v: number) => void;
}

const PX_PER_STEP = 6;
const R = 13;
const pt = (deg: number): [number, number] => [
  17 + R * Math.sin((deg * Math.PI) / 180),
  17 - R * Math.cos((deg * Math.PI) / 180),
];
const arc = (a: number, b: number) => {
  const [x1, y1] = pt(a);
  const [x2, y2] = pt(b);
  return `M${x1} ${y1}A${R} ${R} 0 ${b - a > 180 ? 1 : 0} 1 ${x2} ${y2}`;
};

/** A rotary knob: drag, scroll or arrow keys move 1 dB; Shift makes it 0.1 dB. */
export function Knob(p: Props) {
  const el = useRef<HTMLDivElement>(null);
  const stop = useRef<(() => void) | null>(null);
  const cur = useRef(p);
  cur.current = p;
  const set = (v: number, fine: boolean) => {
    const { min, max, onChange } = cur.current;
    const r = fine ? Math.round(v * 10) / 10 : Math.round(v);
    onChange(Math.min(max, Math.max(min, r)));
  };
  useEffect(() => {
    const node = el.current;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      if (e.deltaY)
        set(cur.current.value + (e.deltaY < 0 ? 1 : -1) * (e.shiftKey ? 0.1 : 1), e.shiftKey);
    };
    node?.addEventListener("wheel", wheel, { passive: false });
    return () => {
      node?.removeEventListener("wheel", wheel);
      stop.current?.();
    };
  }, []);
  const down = (e: PointerEvent) => {
    e.preventDefault();
    // Re-based whenever Shift flips so the value never jumps.
    let fine = e.shiftKey;
    let baseY = e.clientY;
    let baseV = cur.current.value;
    const move = (m: PointerEvent) => {
      if (m.shiftKey !== fine) {
        fine = m.shiftKey;
        baseY = m.clientY;
        baseV = cur.current.value;
      }
      const steps = Math.trunc((baseY - m.clientY) / PX_PER_STEP);
      set(baseV + steps * (fine ? 0.1 : 1), fine);
    };
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
    set(p.value + d * (e.shiftKey ? 0.1 : 1), e.shiftKey);
  };
  const t = (p.value - p.min) / (p.max - p.min || 1);
  const angle = -135 + 270 * Math.min(1, Math.max(0, t));
  const [px, py] = pt(angle);
  return (
    <div
      ref={el}
      role="slider"
      tabIndex={0}
      aria-label={p.name}
      aria-valuemin={p.min}
      aria-valuemax={p.max}
      aria-valuenow={p.value}
      aria-valuetext={`${p.label} when ${fmtDb(p.value)} dB ${p.above ? "above" : "below"}`}
      onPointerDown={down}
      onKeyDown={key}
      onDblClick={() => set(p.def, true)}
      class="flex w-14 cursor-ns-resize select-none flex-col items-center text-small leading-tight outline-none focus-visible:ring-1 focus-visible:ring-accent"
      {...help(`${p.hint} ${H.knob(p.def)}`)}
    >
      <svg width="34" height="34" viewBox="0 0 34 34" aria-hidden="true">
        <path
          d={arc(-135, 135)}
          fill="none"
          class="stroke-border"
          stroke-width="3"
          stroke-linecap="round"
        />
        {t > 0 && (
          <path
            d={arc(-135, angle)}
            fill="none"
            class="stroke-accent"
            stroke-width="3"
            stroke-linecap="round"
          />
        )}
        <line
          x1="17"
          y1="17"
          x2={px}
          y2={py}
          class="stroke-text"
          stroke-width="2"
          stroke-linecap="round"
        />
      </svg>
      <span class="text-dim">{p.label}</span>
      <span class="text-accent">{fmtDb(p.value)} dB</span>
      <span class="text-dim">{p.above ? "above" : "below"}</span>
    </div>
  );
}
