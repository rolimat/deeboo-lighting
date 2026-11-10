import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import type { MomentName } from "../../api";
import type { MomentSet, Order, Palette, SeqRef, Sequence } from "../api";
import { PATTERNS } from "../sequences/SequencesTab";
import { toCells } from "../sequences/cells";
import { roleColour } from "../sequences/RolePicker";
import { canDrop, type DragItem } from "./dnd";

export const ORDERS: [Order, IconName, string][] = [
  ["inOrder", "inorder", "In order"],
  ["random", "random", "Random"],
  ["fixed", "fixed", "Fixed"],
];
const BARS = [1, 2, 4, 8, 16];

/** Segmented icon buttons (order). */
export function OrderPicker({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: Order;
  options: typeof ORDERS;
  onChange: (o: Order) => void;
}) {
  return (
    <span role="group" aria-label={label} class="seg">
      {options.map(([o, icon, name]) => (
        <button
          type="button"
          key={o}
          aria-label={name}
          title={name}
          aria-pressed={o === value}
          onClick={() => onChange(o)}
          class="grid place-items-center px-1.5 py-1"
        >
          <Icon name={icon} />
        </button>
      ))}
    </span>
  );
}

export const selectClass = "rounded border border-border bg-card px-1 py-0.5 text-text";

const patternLabel = (m: MomentName, p: string) => PATTERNS[m].find(([id]) => id === p)?.[1] ?? p;

/** Name, length and a mini segment preview for one clip. */
export function describeRef(
  ref: SeqRef,
  moment: MomentName,
  sequences: [number, Sequence][],
): { name: string; generated: boolean; bars: number | null; roles: string[] } {
  if (ref === "generated") return { name: "Any pattern", generated: true, bars: null, roles: [] };
  if ("pattern" in ref)
    return { name: patternLabel(moment, ref.pattern), generated: true, bars: null, roles: [] };
  const s = sequences.find(([id]) => id === ref.id)?.[1];
  const bars = s ? toCells(s.steps).length / 8 : 0;
  return {
    name: s?.name ?? "Missing sequence",
    generated: false,
    bars,
    roles: s?.steps[0]?.roles ?? [],
  };
}

export interface TrackProps {
  moment: MomentName;
  field: "rhythm" | "calm" | "buildup";
  label: string;
  icon: IconName;
  data: MomentSet;
  palette: Palette | undefined;
  sequences: [number, Sequence][];
  drag: { item: DragItem; name: string } | null;
  /** This moment is the one in force on the live set. */
  playing: boolean;
  /** Entry of `data.sequences` that plays now (live set only) and the loop position 0..1. */
  playingIndex: number | null;
  progress: number;
  onDragClip: (index: number) => void;
  onDragEnd: () => void;
  onDropAt: (index: number) => void;
  onRemove: (index: number) => void;
  onOrder: (o: Order) => void;
  onInterval: (v: string) => void;
  onPreview: (ref: SeqRef, on: boolean) => void;
  onOpen: (ref: SeqRef) => void;
}

/** One moment of a set as an Ableton-style track: options row, clips, drop zone. */
export function Track(p: TrackProps) {
  const target = { kind: "track", moment: p.moment } as const;
  const ok = !p.drag || canDrop(p.drag.item, target);
  const over = (e: Event) => ok && p.drag && e.preventDefault();
  const zoneText = !p.drag
    ? "Drop sequences here"
    : p.drag.item.kind === "palette"
      ? "Sequences only"
      : ok
        ? `Drop "${p.drag.name}" here`
        : `${p.label} only`;
  const bars = p.moment === "calm" ? ["oncePerBreak", ...BARS] : BARS;
  const value = p.data.interval === "oncePerBreak" ? "oncePerBreak" : String(p.data.interval.bars);
  return (
    <section data-track={p.field} aria-label={`${p.label} track`} class="@container min-w-0">
      <h3
        class={`flex items-center gap-2 rounded-t-lg border border-b-0 border-border bg-card px-2 py-1.5 font-semibold ${p.playing ? "text-accent" : ""}`}
      >
        <Icon name={p.icon} />
        {p.label}
      </h3>
      <div class="flex flex-col gap-1.5 border border-t-0 border-border p-1.5">
        <div
          data-options
          class="flex flex-nowrap items-center gap-1 whitespace-nowrap text-[11px] text-dim"
        >
          <span class="hidden @[20rem]:inline">Order</span>
          <OrderPicker
            label={`${p.label} order`}
            value={p.data.order}
            options={ORDERS}
            onChange={p.onOrder}
          />
          <span class="flex-1" />
          <label class="flex min-w-0 items-center gap-1">
            Change every
            <select
              aria-label={`${p.label} change every`}
              value={value}
              onChange={(e) => p.onInterval(e.currentTarget.value)}
              class={selectClass}
            >
              {bars.map((b) => (
                <option key={b} value={String(b)}>
                  {b === "oncePerBreak" ? "once per break" : `${b} ${b === 1 ? "bar" : "bars"}`}
                </option>
              ))}
            </select>
          </label>
        </div>
        {p.data.sequences.map((ref, i) => {
          const d = describeRef(ref, p.moment, p.sequences);
          const playing = p.playingIndex === i;
          const x = (
            <>
              <button
                type="button"
                aria-label={`Preview ${d.name} on strip`}
                title="Hold to preview on the strip"
                onPointerDown={() => p.onPreview(ref, true)}
                onPointerUp={() => p.onPreview(ref, false)}
                onPointerLeave={() => p.onPreview(ref, false)}
                onPointerCancel={() => p.onPreview(ref, false)}
                class="grid place-items-center text-dim"
              >
                <Icon name="preview" />
              </button>
              <button
                type="button"
                aria-label={`Remove ${d.name}`}
                title="Remove from set"
                onClick={() => p.onRemove(i)}
                class="grid place-items-center text-dim"
              >
                <Icon name="close" />
              </button>
            </>
          );
          return (
            <div
              key={i}
              data-clip={`${p.field}-${i}`}
              draggable
              onDragStart={(e) => {
                e.dataTransfer?.setData("text/plain", d.name);
                p.onDragClip(i);
              }}
              onDragEnd={p.onDragEnd}
              onDragOver={over}
              onDrop={(e) => {
                e.preventDefault();
                e.stopPropagation();
                p.onDropAt(i);
              }}
              onDblClick={() => p.onOpen(ref)}
              data-playing={playing ? "" : undefined}
              class={`relative cursor-grab rounded-md border bg-card px-2 py-1.5 ${playing ? "border-accent shadow-[inset_0_0_0_1px_var(--color-accent)]" : "border-border"}`}
            >
              <div class="flex items-center justify-between gap-2">
                <span class="flex min-w-0 items-center gap-1.5">
                  <span class="truncate">{d.name}</span>
                  {d.generated && (
                    <span data-generated class="text-accent" title="Generated from your strip">
                      <Icon name="generated" />
                    </span>
                  )}
                </span>
                <span class="flex shrink-0 items-center gap-1.5 text-dim">
                  {d.bars === null ? "auto" : `${d.bars} ${d.bars === 1 ? "bar" : "bars"}`}
                  {x}
                </span>
              </div>
              {d.roles.length > 0 && (
                <div data-mini class="mt-1 flex gap-0.5">
                  {d.roles.map((role, k) => (
                    <i
                      key={k}
                      class="block h-1.5 flex-1 rounded-sm"
                      style={{
                        background:
                          role !== "off" && p.palette
                            ? roleColour(role, p.palette)
                            : "var(--color-off)",
                      }}
                    />
                  ))}
                </div>
              )}
              {playing && (
                <div
                  data-progress
                  class="absolute bottom-0 left-0 h-0.5 rounded-b-md bg-accent"
                  style={{ width: `${Math.round(p.progress * 100)}%` }}
                />
              )}
            </div>
          );
        })}
        <div
          data-zone
          onDragOver={over}
          onDrop={(e) => {
            e.preventDefault();
            p.onDropAt(p.data.sequences.length);
          }}
          class={`rounded-md border border-dashed px-2 py-2 text-center ${
            !p.drag
              ? "border-border text-dim"
              : ok && p.drag.item.kind !== "palette"
                ? "border-accent bg-accent/10 text-accent"
                : "border-border text-dim opacity-40"
          }`}
        >
          {zoneText}
        </div>
      </div>
    </section>
  );
}
