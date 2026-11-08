import { useEffect, useState } from "preact/hooks";
import { api, type Interval, type Library, type LibrarySet, type MomentSet } from "../api";

const MAX_SETS = 16;
const NAME_MSG = "Name must be 1 to 63 characters, without control characters.";
const INTERVAL_MSG = "Interval must be a whole number from 1 to 999.";
const MOMENTS = [
  ["rhythm", "rhythm", "Rhythm"],
  ["calm", "calm", "Calm"],
  ["buildup", "buildUp", "Build-up"],
] as const;
const ORDERS = [
  ["inOrder", "In order"],
  ["random", "Random"],
  ["fixed", "Fixed"],
] as const;
const UNITS = [
  ["beats", "Beats"],
  ["bars", "Bars"],
  ["phrases", "Phrases"],
] as const;

type Unit = (typeof UNITS)[number][0];
const unitOf = (i: Interval): Unit => ("beats" in i ? "beats" : "bars" in i ? "bars" : "phrases");
const countOf = (i: Interval): number =>
  "beats" in i ? i.beats : "bars" in i ? i.bars : i.phrases;
const intervalOf = (unit: Unit, n: number): Interval =>
  unit === "beats" ? { beats: n } : unit === "bars" ? { bars: n } : { phrases: n };
const intervalOk = (i: Interval) =>
  Number.isInteger(countOf(i)) && countOf(i) >= 1 && countOf(i) <= 999;

const nameOk = (n: string) => {
  const len = [...n].length;
  // eslint-disable-next-line no-control-regex
  return len >= 1 && len <= 63 && !/[\u0000-\u001f\u007f-\u009f]/.test(n);
};

let nextId = 3_000_000;

/** Sets section: per moment, which sequences and palettes a set uses, in what order and how often it changes. */
export function SetEditor() {
  const [lib, setLib] = useState<Library | null>(null);
  const [sets, setSets] = useState<LibrarySet[]>([]);
  const [active, setActive] = useState(0);
  const [sel, setSel] = useState(0);
  const [error, setError] = useState("");

  useEffect(() => {
    api.getLibrary().then(
      (l) => {
        setLib(l);
        setSets(l.sets);
        setActive(l.activeSet);
      },
      (e: unknown) => setError(String(e)),
    );
  }, []);

  const current = sets[sel];
  const edit = (f: (s: LibrarySet) => LibrarySet) =>
    setSets((all) => all.map((s, i) => (i === sel ? f(s) : s)));
  const editMoment = (key: (typeof MOMENTS)[number][0], f: (m: MomentSet) => MomentSet) =>
    edit((s) => ({ ...s, [key]: f(s[key]) }));
  const toggle = <T,>(list: T[], item: T, on: boolean, same: (a: T) => boolean) =>
    on ? [...list, item] : list.filter((x) => !same(x));

  const save = async () => {
    if (!lib) return;
    const badName = sets.findIndex((s) => !nameOk(s.name));
    if (badName >= 0) {
      setSel(badName);
      setError(NAME_MSG);
      return;
    }
    const badInt = sets.findIndex((s) => !MOMENTS.every(([k]) => intervalOk(s[k].interval)));
    if (badInt >= 0) {
      setSel(badInt);
      setError(INTERVAL_MSG);
      return;
    }
    try {
      const saved = await api.saveLibrary({ ...lib, sets, activeSet: active });
      setLib(saved);
      setSets(saved.sets);
      setActive(saved.activeSet);
      setSel((s) => Math.min(s, saved.sets.length - 1));
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  if (!lib) return <p class="text-dim">{error || "Loading…"}</p>;
  const btn = "rounded-card border border-border px-3 py-1 disabled:opacity-40";
  const field = "rounded-card border border-border bg-card px-2 py-1";
  return (
    <div class="space-y-4">
      <div class="flex gap-4">
        <ul class="w-48 space-y-1" aria-label="Sets">
          {sets.map((s, i) => (
            <li key={s.id}>
              <button
                type="button"
                aria-current={i === sel}
                onClick={() => setSel(i)}
                class={`w-full rounded-card px-3 py-2 text-left ${i === sel ? "bg-card" : ""}`}
              >
                {s.name}
              </button>
            </li>
          ))}
        </ul>
        {current && (
          <div class="min-w-0 flex-1 space-y-3">
            <div class="flex gap-2">
              <input
                aria-label="Set name"
                value={current.name}
                onInput={(e) => {
                  const name = e.currentTarget.value;
                  setError(nameOk(name) ? "" : NAME_MSG);
                  edit((s) => ({ ...s, name }));
                }}
                class={field}
              />
              <button
                type="button"
                onClick={() => {
                  const rest = sets.filter((_, i) => i !== sel);
                  setSets(rest);
                  setSel(Math.max(0, sel - 1));
                  // Deleting the last set leaves activeSet dangling on purpose: Rust refuses and says why.
                  if (current.id === active && rest[0]) setActive(rest[0].id);
                }}
                class={btn}
              >
                Delete set
              </button>
            </div>
            {MOMENTS.map(([key, seqMoment, label]) => {
              const m = current[key];
              return (
                <div key={key} class="space-y-2 rounded-card border border-border p-3">
                  <h2>{label}</h2>
                  <fieldset aria-label={`${label} sequences`} class="flex flex-wrap gap-3">
                    {lib.sequences
                      .filter(([, s]) => s.moment === seqMoment)
                      .map(([id, s]) => (
                        <label key={id} class="flex items-center gap-1">
                          <input
                            type="checkbox"
                            aria-label={s.name}
                            checked={m.sequences.some((r) => typeof r === "object" && r.id === id)}
                            onChange={(e) => {
                              const on = e.currentTarget.checked;
                              editMoment(key, (x) => ({
                                ...x,
                                sequences: toggle(
                                  x.sequences,
                                  { id },
                                  on,
                                  (r) => typeof r === "object" && r.id === id,
                                ),
                              }));
                            }}
                          />
                          {s.name}
                        </label>
                      ))}
                  </fieldset>
                  <fieldset aria-label={`${label} palettes`} class="flex flex-wrap gap-3">
                    {lib.palettes.map(([id, p]) => (
                      <label key={id} class="flex items-center gap-1">
                        <input
                          type="checkbox"
                          aria-label={p.name}
                          checked={m.palettes.includes(id)}
                          onChange={(e) => {
                            const on = e.currentTarget.checked;
                            editMoment(key, (x) => ({
                              ...x,
                              palettes: toggle(x.palettes, id, on, (v) => v === id),
                            }));
                          }}
                        />
                        {p.name}
                      </label>
                    ))}
                  </fieldset>
                  <div class="flex items-center gap-2">
                    <select
                      aria-label={`${label} order`}
                      value={m.order}
                      onChange={(e) =>
                        editMoment(key, (x) => ({
                          ...x,
                          order: e.currentTarget.value as MomentSet["order"],
                        }))
                      }
                      class={field}
                    >
                      {ORDERS.map(([v, l]) => (
                        <option key={v} value={v}>
                          {l}
                        </option>
                      ))}
                    </select>
                    <span class="text-dim">every</span>
                    <input
                      type="number"
                      min={1}
                      max={999}
                      aria-label={`${label} interval`}
                      value={countOf(m.interval)}
                      onInput={(e) => {
                        const n = e.currentTarget.valueAsNumber;
                        editMoment(key, (x) => ({
                          ...x,
                          interval: intervalOf(unitOf(x.interval), n),
                        }));
                      }}
                      class={`${field} w-20`}
                    />
                    <select
                      aria-label={`${label} interval unit`}
                      value={unitOf(m.interval)}
                      onChange={(e) =>
                        editMoment(key, (x) => ({
                          ...x,
                          interval: intervalOf(e.currentTarget.value as Unit, countOf(x.interval)),
                        }))
                      }
                      class={field}
                    >
                      {UNITS.map(([v, l]) => (
                        <option key={v} value={v}>
                          {l}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
      <div class="flex gap-2">
        <button
          type="button"
          disabled={sets.length >= MAX_SETS || !current}
          onClick={() => {
            if (!current) return;
            const copy = structuredClone(current);
            setSets((all) => [...all, { ...copy, id: nextId++, name: "New set" }]);
            setSel(sets.length);
          }}
          class={btn}
        >
          New set
        </button>
        <button type="button" onClick={save} class="rounded-card bg-card px-4 py-1">
          Save
        </button>
      </div>
      {error && <p class="text-red">{error}</p>}
    </div>
  );
}
