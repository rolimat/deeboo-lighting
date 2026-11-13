import { useRef, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import type { Palette } from "../api";
import { help } from "../shell/Help";
import { H } from "../help-texts";
import { BAR, MAX_BARS, type Cols } from "./cells";
import { roleColour } from "./RolePicker";

export const ROLE_NAMES: Record<string, string> = {
  principal: "Principal",
  contraste: "Contrast",
  suave: "Soft",
  white: "White",
  off: "Off",
};

const pos = (col: number) =>
  `${Math.floor(col / BAR) + 1}.${Math.floor((col % BAR) / 2) + 1}${col % 2 ? "½" : ""}`;

/**
 * Bar ruler, clip, then one row per segment of half-beat cells. One CSS grid of flexible tracks, so
 * it always fits the window; `zoom` > 1 widens it inside a scroll area instead of squeezing cells.
 */
export function ArrangementGrid({
  cols,
  palette,
  zoom,
  readOnly,
  clip,
  warnings,
  playhead,
  onPaintStart,
  onPaint,
  onDropPalette,
}: {
  cols: Cols;
  palette: Palette;
  zoom: number;
  readOnly: boolean;
  clip: ComponentChildren;
  /** Messages-per-step warnings by column. */
  warnings: Map<number, number>;
  /** 0 to 1 across the loop, or null when not playing. */
  playhead: number | null;
  onPaintStart: () => void;
  onPaint: (seg: number, col: number) => void;
  onDropPalette: (id: number) => void;
}) {
  const painting = useRef(false);
  const [over, setOver] = useState(false);
  const n = cols.length;
  // Empty room after the clip (up to one more bar, never past 16) so the handle can be dragged longer.
  const total = n + Math.min(BAR, Math.max(0, MAX_BARS * BAR - n));
  const segs = cols[0]?.length ?? 0;
  const stop = () => {
    painting.current = false;
    document.removeEventListener("pointerup", stop);
  };
  const focusCell = (seg: number, col: number) =>
    document
      .querySelector<HTMLElement>(`[data-grid] [data-seg="${seg}"][data-col="${col}"]`)
      ?.focus();

  return (
    <div class="flex">
      <div class="w-11 shrink-0 text-small leading-[18px] text-dim">
        <div class="h-[18px]" />
        <div class="h-[30px] leading-[30px]">Clip</div>
        <div class="h-[14px]" />
        {Array.from({ length: segs }, (_, s) => (
          <div key={s} class="h-[18px]">
            S{s + 1}
          </div>
        ))}
      </div>
      <div class="min-w-0 flex-1 overflow-x-auto">
        <div
          data-grid
          class={`relative select-none rounded ${over ? "outline outline-2 outline-accent" : ""}`}
          style={{
            display: "grid",
            gridTemplateColumns: `repeat(${total}, minmax(0, 1fr))`,
            ...(zoom > 1 ? { minWidth: `${zoom * 100}%` } : {}),
          }}
          onDragOver={(e) => {
            if (readOnly) return;
            e.preventDefault();
            setOver(true);
          }}
          onDragLeave={() => setOver(false)}
          onDrop={(e) => {
            e.preventDefault();
            setOver(false);
            const id = Number(e.dataTransfer?.getData("text/plain"));
            if (!readOnly && Number.isFinite(id)) onDropPalette(id);
          }}
        >
          {Array.from({ length: total / 2 }, (_, b) => (
            <div
              key={b}
              style={{ gridColumn: "span 2" }}
              class={`h-[18px] truncate pl-1 text-small text-dim ${b % 4 === 0 ? "font-semibold text-text" : ""}`}
            >
              {b % 4 === 0 ? `Bar ${b / 4 + 1}` : `${Math.floor(b / 4) + 1}.${(b % 4) + 1}`}
            </div>
          ))}
          <div style={{ gridColumn: `1 / span ${n}` }}>{clip}</div>
          <div style={{ gridColumn: "1 / -1" }} class="relative h-[14px]">
            {[...warnings].map(([c, k]) => (
              <span
                key={c}
                role="status"
                data-col={c}
                style={{ left: `${(c / total) * 100}%` }}
                class="absolute whitespace-nowrap text-small leading-[14px] text-red"
              >
                Shown in {k} parts
              </span>
            ))}
          </div>
          {Array.from({ length: segs }, (_, s) => [
            ...cols.map((col, c) => {
              const role = col[s] ?? "off";
              return (
                <button
                  type="button"
                  key={`${s}-${c}`}
                  data-seg={s}
                  {...help(H.cell)}
                  data-col={c}
                  tabIndex={s === 0 && c === 0 ? 0 : -1}
                  aria-label={`Segment ${s + 1}, ${pos(c)}: ${ROLE_NAMES[role] ?? role}`}
                  disabled={readOnly}
                  class="h-[18px] touch-none rounded-[3px] border border-bg p-0"
                  style={{ background: roleColour(role, palette) }}
                  onPointerDown={(e) => {
                    if (e.button) return;
                    painting.current = true;
                    document.addEventListener("pointerup", stop);
                    onPaintStart();
                    onPaint(s, c);
                  }}
                  onPointerEnter={() => painting.current && onPaint(s, c)}
                  onKeyDown={(e) => {
                    const d: Record<string, [number, number]> = {
                      ArrowUp: [-1, 0],
                      ArrowDown: [1, 0],
                      ArrowLeft: [0, -1],
                      ArrowRight: [0, 1],
                    };
                    const m = d[e.key];
                    if (m) {
                      e.preventDefault();
                      focusCell(
                        Math.min(segs - 1, Math.max(0, s + m[0])),
                        Math.min(n - 1, Math.max(0, c + m[1])),
                      );
                    } else if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onPaintStart();
                      onPaint(s, c);
                    }
                  }}
                />
              );
            }),
            // Empty room after the clip: one placeholder per row so the row never wraps.
            ...Array.from({ length: total - n }, (_, k) => (
              <div
                key={`${s}-room-${k}`}
                data-room
                aria-hidden="true"
                class="h-[18px] rounded-[3px] border border-bg bg-card opacity-30"
              />
            )),
          ])}
          <div aria-hidden="true" class="pointer-events-none absolute inset-0">
            {Array.from({ length: total / 2 }, (_, b) => (
              <div
                key={b}
                style={{ left: `${((b * 2) / total) * 100}%` }}
                class={`absolute inset-y-0 ${b % 4 === 0 ? "w-0.5 bg-[#7a808a]" : "w-px bg-border"}`}
              />
            ))}
            {playhead !== null && (
              <div
                data-playhead
                style={{ left: `${((playhead * n) / total) * 100}%` }}
                class="absolute -top-1 bottom-0 z-10 w-0.5 bg-accent"
              />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
