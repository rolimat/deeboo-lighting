import { Icon } from "../../icons/Icon";
import { help } from "../shell/Help";
import { H } from "../help-texts";

export interface StripInfo {
  name: string;
  /** Wired segments. */
  segments: number;
  /** What the controller reports. */
  reported: number;
  /** Private LAN address, when known. */
  ip?: string;
}

const chip = "rounded-lg border border-border bg-bg px-3 py-1";
const iconChip = `${chip} inline-flex items-center gap-1.5`;

/** The connected strip: its state, how many segments are wired, and the way back into the wizard. */
export function SegmentsCard(p: {
  strip: StripInfo;
  wired: number;
  confirm: number | null;
  onStep: (n: number) => void;
  onConfirm: (yes: boolean) => void;
  onNumber: (n: number) => void;
  onRepair: () => void;
}) {
  const { name, reported } = p.strip;
  const step = "grid h-7 w-7 place-items-center disabled:opacity-40";
  return (
    <div class="mb-3 rounded-card border border-border bg-card p-3">
      <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
        <span class="text-accent">
          <Icon name="wifi" />
        </span>
        <div>
          <b>{name}</b>
          <div class="text-small text-dim">
            {p.strip.ip ? `Connected · ${p.strip.ip}` : "Connected"}
          </div>
        </div>
        <div class="flex items-center gap-2">
          <span class="t-label">Segments</span>
          <span class="inline-flex items-center overflow-hidden rounded-lg border border-border">
            <button
              type="button"
              aria-label="Fewer segments"
              {...help(H.fewerSegments)}
              disabled={p.wired <= 1}
              onClick={() => p.onStep(p.wired - 1)}
              class={step}
            >
              −
            </button>
            <output aria-label="Wired segments" class="bg-bg px-3 py-1">
              {p.wired}
            </output>
            <button
              type="button"
              aria-label="More segments"
              {...help(H.moreSegments)}
              disabled={p.wired >= reported}
              onClick={() => p.onStep(p.wired + 1)}
              class={step}
            >
              +
            </button>
          </span>
        </div>
        <div class="flex flex-wrap gap-1">
          {Array.from({ length: reported }, (_, i) => i + 1).map((n) => (
            <button
              key={n}
              type="button"
              data-segment-number
              data-wired={String(n <= p.wired)}
              aria-label={`Light segment ${n}`}
              {...help(H.lightSegment)}
              onClick={() => p.onNumber(n)}
              class={`grid h-6 w-7 place-items-center rounded-md border text-small ${n <= p.wired ? "border-border bg-bg" : "border-dashed border-border opacity-40"}`}
            >
              {n}
            </button>
          ))}
        </div>
        <span class="flex-1" />
        <button type="button" class={iconChip} onClick={p.onRepair} {...help(H.reconnect)}>
          <Icon name="refresh" />
          Reconnect…
        </button>
        <button type="button" class={iconChip} onClick={p.onRepair} {...help(H.changeStrip)}>
          <Icon name="swap" />
          Change strip…
        </button>
      </div>
      <p class="mt-2 text-small text-dim">
        {`The controller reports ${reported}; only ${p.wired} are wired. Click a number to light that segment and count.`}
      </p>
      {p.confirm !== null && (
        <div role="alertdialog" class="mt-3 flex items-center gap-3 border-t border-border pt-3">
          <span>{`Reduce to ${p.confirm} segments? Sequences lose segments ${p.confirm + 1}–${p.wired}.`}</span>
          <button
            type="button"
            class={chip}
            onClick={() => p.onConfirm(false)}
            {...help(H.cancelReduce)}
          >
            Cancel
          </button>
          <button
            type="button"
            class="rounded-lg bg-red px-3 py-1 text-white"
            onClick={() => p.onConfirm(true)}
            {...help(H.confirmReduce)}
          >
            Reduce
          </button>
        </div>
      )}
    </div>
  );
}
