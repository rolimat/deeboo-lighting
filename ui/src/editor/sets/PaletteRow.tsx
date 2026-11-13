import { Icon } from "../../icons/Icon";
import { help } from "../shell/Help";
import { H } from "../help-texts";
import type { LibrarySet, Order, PaletteChange, Palette } from "../api";
import { css } from "../sequences/RolePicker";
import { canDrop, type DragItem } from "./dnd";
import { ORDERS, OrderPicker, selectClass } from "./Track";

const BARS = [1, 2, 4, 8, 16];
const OPTIONS = ORDERS.filter(([o]) => o !== "fixed");

export const Swatch = ({ p }: { p: Palette }) => (
  <span class="inline-flex gap-px">
    {[p.principal, p.contraste, p.suave].map((c, i) => (
      <i key={i} class="block h-2.5 w-2.5 rounded-sm" style={{ background: css(c) }} />
    ))}
  </span>
);

/** The set's palettes: one full-width row under the tracks, chips wrapping, drop zone after the last chip. */
export function PaletteRow({
  set,
  palettes,
  playingName,
  drag,
  onDrop,
  onRemove,
  onOrder,
  onChange,
}: {
  set: LibrarySet;
  palettes: [number, Palette][];
  playingName: string | null;
  drag: { item: DragItem; name: string } | null;
  onDrop: () => void;
  onRemove: (id: number) => void;
  onOrder: (o: Order) => void;
  onChange: (c: PaletteChange) => void;
}) {
  const ok = !drag || canDrop(drag.item, { kind: "palettes" });
  const value = set.paletteChange === "atDrop" ? "atDrop" : String(set.paletteChange.bars);
  return (
    <div data-palette-row class="shrink-0 rounded-lg border border-border bg-card/40 p-2">
      <div class="mb-1.5 flex items-center gap-2 text-small text-dim">
        <b class="text-text">Palettes</b> · change
        <select
          aria-label="Palette change"
          {...help(H.paletteChange)}
          value={value}
          onChange={(e) => {
            const v = e.currentTarget.value;
            onChange(v === "atDrop" ? "atDrop" : { bars: Number(v) });
          }}
          class={selectClass}
        >
          <option value="atDrop">at each drop</option>
          {BARS.map((b) => (
            <option key={b} value={String(b)}>{`every ${b} ${b === 1 ? "bar" : "bars"}`}</option>
          ))}
        </select>
        <OrderPicker
          label="Palette order"
          value={set.paletteOrder}
          options={OPTIONS}
          onChange={onOrder}
        />
      </div>
      <div data-chips class="flex flex-wrap items-stretch gap-1">
        {set.palettes.map((id) => {
          const p = palettes.find(([i]) => i === id)?.[1];
          if (!p) return null;
          const on = p.name === playingName;
          return (
            <span
              key={id}
              data-chip
              class={`inline-flex items-center gap-1.5 rounded border bg-card px-1.5 py-0.5 ${on ? "border-accent" : "border-border"}`}
            >
              <Swatch p={p} />
              {p.name}
              {on && (
                <span class="text-accent" {...help(H.playingNow)}>
                  <Icon name="play" />
                </span>
              )}
              <button
                type="button"
                aria-label={`Remove ${p.name}`}
                {...help(H.removePalette)}
                onClick={() => onRemove(id)}
                class="grid place-items-center text-dim"
              >
                <Icon name="close" />
              </button>
            </span>
          );
        })}
        <div
          data-palette-drop
          {...help(H.paletteDrop)}
          onDragOver={(e) => ok && drag && e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            onDrop();
          }}
          class={`min-w-36 flex-1 rounded border border-dashed px-3 py-0.5 text-center ${
            !drag
              ? "border-border text-dim"
              : ok
                ? "border-accent bg-accent/10 text-accent"
                : "border-border text-dim opacity-40"
          }`}
        >
          {!drag ? "Drop palettes here" : ok ? `Drop "${drag.name}" here` : "Palettes only"}
        </div>
      </div>
    </div>
  );
}
