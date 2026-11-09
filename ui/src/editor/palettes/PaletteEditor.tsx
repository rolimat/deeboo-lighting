import { useEffect, useState } from "preact/hooks";
import { api, type Hsv, type Library, type Palette } from "../api";
import { css } from "../sequences/RolePicker";

const MAX_PALETTES = 32;
const NAME_MSG = "Name must be 1 to 63 characters, without control characters.";
const COLOURS = [
  ["principal", "Principal"],
  ["contraste", "Contraste"],
  ["suave", "Suave"],
] as const;
const CHANNELS = [
  ["h", "hue", 360],
  ["s", "saturation", 1000],
  ["v", "value", 1000],
] as const;

const nameOk = (n: string) => {
  const len = [...n].length;
  // eslint-disable-next-line no-control-regex
  return len >= 1 && len <= 63 && !/[\u0000-\u001f\u007f-\u009f]/.test(n);
};

let nextId = 2_000_000;

/** Palettes section: list and the three colours of each palette. Saving goes through `save_library`. */
export function PaletteEditor() {
  const [lib, setLib] = useState<Library | null>(null);
  const [pals, setPals] = useState<[number, Palette][]>([]);
  const [sel, setSel] = useState(0);
  const [error, setError] = useState("");

  useEffect(() => {
    api.getLibrary().then(
      (l) => {
        setLib(l);
        setPals(l.palettes);
      },
      (e: unknown) => setError(String(e)),
    );
  }, []);

  const current = pals[sel]?.[1];
  const update = (f: (p: Palette) => Palette) =>
    setPals((all) => all.map(([id, p], i) => (i === sel ? [id, f(p)] : [id, p])));

  const save = async () => {
    if (!lib) return;
    const bad = pals.findIndex(([, p]) => !nameOk(p.name));
    if (bad >= 0) {
      setSel(bad);
      setError(NAME_MSG);
      return;
    }
    try {
      // Fresh copy: the popover may have changed the active set or palette since this loaded.
      const fresh = await api.getLibrary();
      const active = fresh.activePalette;
      const saved = await api.saveLibrary({
        ...fresh,
        palettes: pals,
        activePalette: pals.some(([id]) => id === active) ? active : null,
      });
      setLib(saved);
      setPals(saved.palettes);
      setSel((s) => Math.min(s, saved.palettes.length - 1));
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  if (!lib) return <p class="text-dim">{error || "Loading…"}</p>;
  const btn = "rounded-card border border-border px-3 py-1 disabled:opacity-40";
  const usedBy = lib.sets
    .filter((set) => set.palettes.includes(pals[sel]?.[0] ?? -1))
    .map((set) => set.name);
  return (
    <div class="space-y-4">
      <div class="flex gap-4">
        <ul class="w-48 space-y-1" aria-label="Palettes">
          {pals.map(([id, p], i) => (
            <li key={id}>
              <button
                type="button"
                aria-current={i === sel}
                onClick={() => setSel(i)}
                class={`w-full rounded-card px-3 py-2 text-left ${i === sel ? "bg-card" : ""}`}
              >
                {p.name}
              </button>
            </li>
          ))}
        </ul>
        {current && (
          <div class="min-w-0 flex-1 space-y-3">
            <input
              aria-label="Palette name"
              value={current.name}
              onInput={(e) => {
                const name = e.currentTarget.value;
                setError(nameOk(name) ? "" : NAME_MSG);
                update((p) => ({ ...p, name }));
              }}
              class="rounded-card border border-border bg-card px-2 py-1"
            />
            {COLOURS.map(([key, label]) => (
              <fieldset key={key} class="flex items-center gap-3" aria-label={label}>
                <span
                  aria-hidden="true"
                  class="inline-block h-8 w-8 rounded-full border border-border"
                  style={{ background: css(current[key]) }}
                />
                <span class="w-20">{label}</span>
                {CHANNELS.map(([c, name, max]) => (
                  <label key={c} class="flex items-center gap-1 text-dim">
                    {name}
                    <input
                      type="range"
                      min={0}
                      max={max}
                      aria-label={`${label} ${name}`}
                      value={current[key][c]}
                      onInput={(e) => {
                        const n = Number(e.currentTarget.value);
                        update((p) => ({ ...p, [key]: { ...p[key], [c]: n } as Hsv }));
                      }}
                    />
                  </label>
                ))}
              </fieldset>
            ))}
            <p class="text-dim">Suave: use a light colour for Calm.</p>
            <button
              type="button"
              disabled={pals.length <= 1 || usedBy.length > 0}
              onClick={() => {
                setPals((all) => all.filter((_, i) => i !== sel));
                setSel(Math.max(0, sel - 1));
              }}
              class={btn}
            >
              Delete palette
            </button>
            {usedBy.length > 0 && (
              <p class="text-dim">
                Used by {usedBy.length > 1 ? "sets" : "set"} {usedBy.join(", ")} {"\u2014"} remove
                it there first.
              </p>
            )}
          </div>
        )}
      </div>
      <div class="flex gap-2">
        <button
          type="button"
          disabled={pals.length >= MAX_PALETTES}
          onClick={() => {
            const base = current ?? {
              name: "",
              principal: { h: 0, s: 1000, v: 1000 },
              contraste: { h: 240, s: 1000, v: 1000 },
              suave: { h: 120, s: 300, v: 1000 },
            };
            setPals((all) => [...all, [nextId++, { ...base, name: "New palette" }]]);
            setSel(pals.length);
          }}
          class={btn}
        >
          New palette
        </button>
        <button type="button" onClick={save} class="rounded-card bg-card px-4 py-1">
          Save
        </button>
      </div>
      {error && <p class="text-red">{error}</p>}
    </div>
  );
}
