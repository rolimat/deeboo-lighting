import { help } from "../shell/Help";
import { H } from "../help-texts";

/** Five steps of "Moment changes" <-> detector sensitivity 1..10 (shared with Setup). */
const SENSITIVITY = [1, 3, 5, 7, 9];
const EXPLAIN = [
  "Waits for big changes before moving to Calm or Build-up.",
  "Changes a bit later than Balanced.",
  "Moves to Calm or Build-up only when the music clearly changes.",
  "Changes a bit sooner than Balanced.",
  "Follows small changes in the music.",
];
const NAMES: [number, string][] = [
  [0, "Steady"],
  [2, "Balanced"],
  [4, "Reactive"],
];

export const sensitivityOf = (step: number): number => SENSITIVITY[step] ?? 5;
/** Nearest step; ties go to the gentler one. */
export const stepOf = (sensitivity: number): number =>
  SENSITIVITY.reduce(
    (best, v, i) =>
      Math.abs(v - sensitivity) < Math.abs((SENSITIVITY[best] ?? 0) - sensitivity) ? i : best,
    0,
  );

export function ChangesSlider(props: {
  sensitivity: number;
  onChange: (sensitivity: number) => void;
}) {
  const step = stepOf(props.sensitivity);
  return (
    <div class="w-[340px]">
      <input
        type="range"
        aria-label="Moment changes"
        {...help(H.momentChanges)}
        min={0}
        max={4}
        step={1}
        value={step}
        onInput={(e) => props.onChange(sensitivityOf(Number(e.currentTarget.value)))}
        class="w-full accent-accent"
      />
      <div aria-hidden="true" class="flex justify-between px-[7px]">
        {SENSITIVITY.map((v, i) => (
          <i key={v} class={`h-2 w-0.5 ${i <= step ? "bg-accent" : "bg-border"}`} />
        ))}
      </div>
      <div class="mt-1 flex justify-between text-small text-dim">
        {NAMES.map(([i, name]) =>
          i === step ? (
            <b key={name} data-current class="text-text">
              {name}
            </b>
          ) : (
            <span key={name}>{name}</span>
          ),
        )}
      </div>
      <p class="mt-1 text-small text-dim">{EXPLAIN[step]}</p>
    </div>
  );
}
