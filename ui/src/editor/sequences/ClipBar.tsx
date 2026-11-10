import { useRef } from "preact/hooks";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import { MAX_BARS } from "./cells";

/** The sequence as one clip: its length in whole bars, stretched from the right edge. */
export function ClipBar({
  name,
  icon,
  bars,
  readOnly,
  onBars,
}: {
  name: string;
  icon: IconName;
  bars: number;
  readOnly: boolean;
  onBars: (bars: number) => void;
}) {
  const track = useRef<HTMLDivElement>(null);
  const clamp = (n: number) => Math.min(MAX_BARS, Math.max(1, n));

  const drag = (e: PointerEvent) => {
    const r = track.current?.getBoundingClientRect();
    if (!r || !r.width) return;
    e.preventDefault();
    const perBar = r.width / bars; // fixed for this drag, so the handle does not jitter as the grid rescales
    const move = (m: PointerEvent) => onBars(clamp(Math.round((m.clientX - r.left) / perBar)));
    const up = () => {
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", up);
    };
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", up);
  };

  return (
    <div ref={track} data-clip-track class="relative h-[30px] py-0.5">
      <div
        data-clip
        class="relative flex h-full items-center gap-2 rounded-md border border-accent bg-card pl-2 pr-4 text-xs font-semibold"
      >
        <span class="text-dim">
          <Icon name={icon} />
        </span>
        <span class="truncate">{`${name} · ${bars} ${bars === 1 ? "bar" : "bars"} · loops`}</span>
        {!readOnly && (
          <div
            role="slider"
            tabIndex={0}
            aria-label="Clip length"
            aria-valuemin={1}
            aria-valuemax={MAX_BARS}
            aria-valuenow={bars}
            aria-valuetext={`${bars} ${bars === 1 ? "bar" : "bars"}`}
            title="Drag to change the length (whole bars)"
            onPointerDown={drag}
            onKeyDown={(e) => {
              const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
              if (!d) return;
              e.preventDefault();
              onBars(clamp(bars + d));
            }}
            class="absolute -bottom-px -right-px -top-px w-2 cursor-ew-resize touch-none rounded-r-md bg-accent"
          />
        )}
      </div>
    </div>
  );
}
