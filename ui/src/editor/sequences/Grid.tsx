import { useEffect, useRef } from "preact/hooks";
import type { Palette, Step } from "../api";
import { ROLE_LABEL, roleColour, type Role } from "./RolePicker";

export const MAX_STEPS = 64;
const LENS: [string, string][] = [
  ["half", "½ beat"],
  ["one", "1 beat"],
  ["two", "2 beats"],
  ["four", "4 beats"],
];

/** Rows are segments, columns steps; pointer click/drag or keyboard paints the chosen role. */
export function Grid({
  steps,
  palette,
  role,
  counts,
  onChange,
}: {
  steps: Step[];
  palette: Palette;
  role: Role;
  /** Messages per step; a step costing more than one is shown in parts. */
  counts: number[];
  onChange: (steps: Step[]) => void;
}) {
  const painting = useRef(false);
  const latest = useRef(steps);
  latest.current = steps;
  useEffect(() => {
    const stop = () => (painting.current = false);
    document.addEventListener("pointerup", stop);
    return () => document.removeEventListener("pointerup", stop);
  }, []);

  const paint = (seg: number, col: number) =>
    onChange(
      latest.current.map((s, c) =>
        c === col ? { ...s, roles: s.roles.map((r, i) => (i === seg ? role : r)) } : s,
      ),
    );
  const edit = (col: number, f: (s: Step) => Step) =>
    onChange(latest.current.map((s, c) => (c === col ? f(s) : s)));
  const insert = (at: number, step: Step) =>
    onChange([...latest.current.slice(0, at), step, ...latest.current.slice(at)]);
  const full = steps.length >= MAX_STEPS;
  const segments = steps[0]?.roles.length ?? 0;

  return (
    <div class="overflow-x-auto">
      <div class="flex gap-1">
        <div class="w-16" />
        {steps.map((s, c) => (
          <div key={c} class="flex w-24 flex-col gap-1 text-sm">
            <select
              aria-label={`Step ${c + 1} length`}
              value={s.len}
              onChange={(e) => edit(c, (st) => ({ ...st, len: e.currentTarget.value }))}
              class="rounded-card border border-border bg-card px-1"
            >
              {LENS.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
            <div class="flex gap-1">
              <button
                type="button"
                aria-label={`Duplicate step ${c + 1}`}
                disabled={full}
                onClick={() => insert(c + 1, { ...s, roles: [...s.roles] })}
                class="flex-1 rounded-card border border-border disabled:opacity-40"
              >
                ⧉
              </button>
              <button
                type="button"
                aria-label={`Remove step ${c + 1}`}
                disabled={steps.length <= 1}
                onClick={() => onChange(latest.current.filter((_, i) => i !== c))}
                class="flex-1 rounded-card border border-border disabled:opacity-40"
              >
                ✕
              </button>
            </div>
            {(counts[c] ?? 0) > 1 && (
              <p class="text-red" role="status">
                Shown in {counts[c]} parts
              </p>
            )}
          </div>
        ))}
      </div>
      {Array.from({ length: segments }, (_, seg) => (
        <div key={seg} class="mt-1 flex gap-1">
          <div class="w-16 py-2 text-sm text-dim">Segment {seg + 1}</div>
          {steps.map((s, c) => {
            const r = s.roles[seg] ?? "off";
            return (
              <button
                type="button"
                key={c}
                aria-label={`Segment ${seg + 1}, step ${c + 1}: ${ROLE_LABEL[r as Role] ?? r}`}
                class="h-8 w-24 touch-none rounded-card border border-border"
                style={{ background: roleColour(r, palette) }}
                onPointerDown={() => {
                  painting.current = true;
                  paint(seg, c);
                }}
                onPointerEnter={() => painting.current && paint(seg, c)}
                onClick={(e) => e.detail === 0 && paint(seg, c)} // keyboard; pointer already painted
              />
            );
          })}
        </div>
      ))}
    </div>
  );
}
