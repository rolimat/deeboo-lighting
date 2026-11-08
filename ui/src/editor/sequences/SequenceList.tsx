import { useEffect, useRef, useState } from "preact/hooks";
import { api, type Library, type Sequence, type Step } from "../api";
import { Grid, MAX_STEPS } from "./Grid";
import { Simulation } from "./Simulation";
import { RolePicker, type Role } from "./RolePicker";

const MAX_SEQUENCES = 64;
const MOMENTS: [string, string][] = [
  ["rhythm", "Rhythm"],
  ["calm", "Calm"],
  ["buildUp", "Build-up"],
];
const NAME_MSG = "Name must be 1 to 63 characters, without control characters.";
const DEBOUNCE_MS = 150;

const nameOk = (n: string) => {
  const len = [...n].length;
  // eslint-disable-next-line no-control-regex
  return len >= 1 && len <= 63 && !/[\u0000-\u001f\u007f-\u009f]/.test(n);
};

let nextId = 1_000_000;

/** Sequences section: list, grid editor and message warnings. Saving goes through `save_library`. */
export function SequenceList() {
  const [lib, setLib] = useState<Library | null>(null);
  const [seqs, setSeqs] = useState<[number, Sequence][]>([]);
  const [sel, setSel] = useState(0);
  const [paletteId, setPaletteId] = useState(0);
  const [role, setRole] = useState<Role>("principal");
  const [counts, setCounts] = useState<number[]>([]);
  const [error, setError] = useState("");
  const [nameError, setNameError] = useState("");

  useEffect(() => {
    api.getLibrary().then(
      (l) => {
        setLib(l);
        setSeqs(l.sequences);
        setPaletteId(l.activePalette ?? l.palettes[0]?.[0] ?? 0);
      },
      (e: unknown) => setError(String(e)),
    );
  }, []);

  const current = seqs[sel]?.[1];
  const palette = lib?.palettes.find(([id]) => id === paletteId)?.[1];

  // Debounced: one call after a burst of edits (drag paints many cells).
  const seen = useRef(0);
  useEffect(() => {
    if (!current) return;
    const ticket = ++seen.current;
    const t = setTimeout(() => {
      api.messageCounts(current, paletteId).then(
        (c) => ticket === seen.current && setCounts(c),
        () => ticket === seen.current && setCounts([]),
      );
    }, DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [current?.steps, paletteId]);

  const update = (f: (s: Sequence) => Sequence) =>
    setSeqs((all) => all.map(([id, s], i) => (i === sel ? [id, f(s)] : [id, s])));
  const add = (s: Sequence) => {
    setSeqs((all) => [...all, [nextId++, s]]);
    setSel(seqs.length);
  };
  const blank = (): Sequence => ({
    name: "New sequence",
    moment: "rhythm",
    steps: [
      {
        len: "one",
        roles: Array.from({ length: lib?.shape.pieces.length ?? 0 }, () => "principal"),
      },
    ],
  });

  const save = async () => {
    if (!lib) return;
    const bad = seqs.findIndex(([, s]) => !nameOk(s.name));
    if (bad >= 0) {
      setSel(bad);
      setNameError(NAME_MSG);
      return;
    }
    try {
      const saved = await api.saveLibrary({ ...lib, sequences: seqs });
      setLib(saved);
      setSeqs(saved.sequences);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  if (!lib) return <p class="text-dim">{error || "Loading…"}</p>;
  return (
    <div class="space-y-4">
      <div class="flex gap-4">
        <ul class="w-48 space-y-1" aria-label="Sequences">
          {seqs.map(([id, s], i) => (
            <li key={id}>
              <button
                type="button"
                aria-current={i === sel}
                onClick={() => {
                  setSel(i);
                  setNameError("");
                }}
                class={`w-full rounded-card px-3 py-2 text-left ${i === sel ? "bg-card" : ""}`}
              >
                {s.name}
              </button>
            </li>
          ))}
        </ul>
        {current && palette ? (
          <div class="min-w-0 flex-1 space-y-3">
            <div class="flex flex-wrap items-center gap-2">
              <input
                aria-label="Sequence name"
                value={current.name}
                onInput={(e) => {
                  const name = e.currentTarget.value;
                  setNameError(nameOk(name) ? "" : NAME_MSG);
                  update((s) => ({ ...s, name }));
                }}
                class="rounded-card border border-border bg-card px-2 py-1"
              />
              <select
                aria-label="Moment"
                value={current.moment}
                onChange={(e) => update((s) => ({ ...s, moment: e.currentTarget.value }))}
                class="rounded-card border border-border bg-card px-2 py-1"
              >
                {MOMENTS.map(([v, l]) => (
                  <option key={v} value={v}>
                    {l}
                  </option>
                ))}
              </select>
              <select
                aria-label="Palette"
                value={paletteId}
                onChange={(e) => setPaletteId(Number(e.currentTarget.value))}
                class="rounded-card border border-border bg-card px-2 py-1"
              >
                {lib.palettes.map(([id, p]) => (
                  <option key={id} value={id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </div>
            {nameError && <p class="text-red">{nameError}</p>}
            <RolePicker palette={palette} value={role} onChange={setRole} />
            <Grid
              steps={current.steps}
              palette={palette}
              role={role}
              counts={counts}
              onChange={(steps: Step[]) => update((s) => ({ ...s, steps }))}
            />
            <Simulation sequence={current} pieces={lib.shape.pieces} palette={palette} />
            <div class="flex gap-2">
              <button
                type="button"
                disabled={current.steps.length >= MAX_STEPS}
                onClick={() =>
                  update((s) => ({
                    ...s,
                    steps: [
                      ...s.steps,
                      { len: "one", roles: s.steps[0]?.roles.map(() => "off") ?? [] },
                    ],
                  }))
                }
                class="rounded-card border border-border px-3 py-1 disabled:opacity-40"
              >
                Add step
              </button>
              <button
                type="button"
                disabled={seqs.length >= MAX_SEQUENCES}
                onClick={() =>
                  add({
                    ...current,
                    name: [...`${current.name} copy`].slice(0, 63).join(""),
                    steps: current.steps.map((s) => ({ ...s, roles: [...s.roles] })),
                  })
                }
                class="rounded-card border border-border px-3 py-1 disabled:opacity-40"
              >
                Duplicate sequence
              </button>
              <button
                type="button"
                disabled={seqs.length <= 1}
                onClick={() => {
                  setSeqs((all) => all.filter((_, i) => i !== sel));
                  setSel(Math.max(0, sel - 1));
                }}
                class="rounded-card border border-border px-3 py-1 disabled:opacity-40"
              >
                Delete sequence
              </button>
            </div>
          </div>
        ) : (
          <p class="text-dim">No palette to show colours with.</p>
        )}
      </div>
      <div class="flex gap-2">
        <button
          type="button"
          disabled={seqs.length >= MAX_SEQUENCES}
          onClick={() => add(blank())}
          class="rounded-card border border-border px-3 py-1 disabled:opacity-40"
        >
          New sequence
        </button>
        <button type="button" onClick={save} class="rounded-card bg-card px-4 py-1">
          Save
        </button>
      </div>
      {error && <p class="text-red">{error}</p>}
    </div>
  );
}
