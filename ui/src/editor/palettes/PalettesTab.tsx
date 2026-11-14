import { COLOUR_TYPE, ROLE_TYPE } from "../drag";
import { useEffect, useRef, useState } from "preact/hooks";
import { api, type Hsv, type Palette } from "../api";
import { Icon } from "../../icons/Icon";
import { help } from "../shell/Help";
import { H } from "../help-texts";
import type { BarActions } from "../sequences/SequencesTab";
import { css } from "../sequences/RolePicker";
import { Simulation } from "../sequences/Simulation";
import type { useLibrary } from "../state/library";
import { ColourWheel } from "./ColourWheel";
import { fromHex, pastelPalette, toHex } from "./colour";

type Store = ReturnType<typeof useLibrary>;
type RoleKey = "principal" | "contraste" | "suave";

const MAX_PALETTES = 32;
const NAME_MSG = "Name must be 1 to 63 characters, without control characters.";
const HEX_MSG = "Enter a colour like #FF2D55.";
const PREVIEW_DEBOUNCE_MS = 120;
const ROLES: [RoleKey, string, string][] = [
  ["principal", "Principal", "Main colour · most segments"],
  ["contraste", "Contrast", "Accents and hits"],
  ["suave", "Soft", "Background, calm moments"],
];
const QUICK = [
  "#FF0000",
  "#FF5E00",
  "#FFB000",
  "#FFEE00",
  "#7DFF00",
  "#00FF6A",
  "#00FFD5",
  "#00A2ff",
  "#2D3CFF",
  "#8A2DFF",
  "#FF2DD2",
  "#FF2D55",
  "#FFFFFF",
  "#FFE0B2",
];

const nameOk = (n: string) => {
  const len = [...n].length;
  // eslint-disable-next-line no-control-regex
  return len >= 1 && len <= 63 && !/[\u0000-\u001f\u007f-\u009f]/.test(n);
};

type Drag = { kind: "role"; role: RoleKey } | { kind: "colour" } | null;

/** Palettes tab: browser, three role colours, a wheel with one brightness slider, and a strip preview. */
export function PalettesTab({
  store,
  onActions,
  onGoSets,
}: {
  store: Store;
  onActions: (a: BarActions | null) => void;
  /** A «Used by» chip was clicked. */
  onGoSets?: () => void;
}) {
  const { lib, edit } = store;
  const [sel, setSel] = useState<number | null>(null);
  const [role, setRole] = useState<RoleKey>("principal");
  const [q, setQ] = useState("");
  const [draft, setDraft] = useState<string | null>(null);
  const [hexDraft, setHexDraft] = useState<string | null>(null);
  const [hexBad, setHexBad] = useState(false);
  const [notice, setNotice] = useState("");
  const [drag, setDrag] = useState<Drag>(null);
  const [seqId, setSeqId] = useState<number | null>(null);
  useEffect(
    () =>
      store.onRemap((m) => {
        setSel((s) => (s === null ? s : (m.palettes.get(s) ?? s)));
        setSeqId((s) => (s === null ? s : (m.sequences.get(s) ?? s)));
      }),
    [store.onRemap],
  );
  const [onStrip, setOnStrip] = useState(false);
  const live = useRef<{ dup: () => void; del: () => void; guard: () => boolean }>({
    dup: () => {},
    del: () => {},
    guard: () => false,
  });

  const pickedId =
    sel !== null && lib?.palettes.some(([id]) => id === sel)
      ? sel
      : (lib?.palettes[0]?.[0] ?? null);
  const palette = lib?.palettes.find(([id]) => id === pickedId)?.[1];
  const sequences = lib?.sequences ?? [];
  const previewEntry = sequences.find(([id]) => id === seqId) ?? sequences[0];
  const previewSeq = previewEntry?.[0] ?? null;
  const users = lib?.sets.filter((s) => pickedId !== null && s.palettes.includes(pickedId)) ?? [];

  const setPalette = (id: number, fn: (p: Palette) => Palette, coalesce?: string) =>
    edit(
      (l) => ({ ...l, palettes: l.palettes.map(([i, p]) => (i === id ? [i, fn(p)] : [i, p])) }),
      { coalesce },
    );
  const newId = () => Math.max(1_000_000, ...(lib?.palettes.map(([id]) => id) ?? [])) + 1;
  const add = (p: Palette) => {
    if ((lib?.palettes.length ?? 0) >= MAX_PALETTES) return setNotice("At most 32 palettes.");
    const id = newId();
    edit((l) => ({ ...l, palettes: [...l.palettes, [id, p]] }));
    setSel(id);
    setDraft(null);
    setNotice("");
  };
  live.current = {
    dup: () =>
      palette && add({ ...palette, name: [...`${palette.name} copy`].slice(0, 63).join("") }),
    guard: () => {
      if (!users.length) return false;
      setNotice(`Used by ${users.map((u) => u.name).join(", ")}. Remove it from there first.`);
      return true;
    },
    del: () => {
      if (pickedId === null) return;
      edit((l) => ({
        ...l,
        palettes: l.palettes.filter(([id]) => id !== pickedId),
        activePalette: l.activePalette === pickedId ? null : l.activePalette,
      }));
      setSel(null);
      setNotice("");
    },
  };
  const canAct = !!palette;
  const label = palette?.name;
  useEffect(() => {
    onActions(
      canAct
        ? {
            onDuplicate: () => live.current.dup(),
            onDelete: () => live.current.del(),
            deleteGuard: () => live.current.guard(),
            deleteLabel: label,
          }
        : {},
    );
  }, [canAct, label]);
  useEffect(() => () => onActions(null), []);

  // "Show on strip": follows the palette being edited; always off when leaving.
  const lastOn = useRef<{ palette: Palette; seq: number } | null>(null);
  useEffect(() => {
    if (!onStrip || !palette || previewSeq === null) return;
    const seq = previewSeq;
    const t = setTimeout(() => {
      lastOn.current = { palette, seq };
      api.previewPalette(true, palette, seq).catch((e: unknown) => {
        setOnStrip(false);
        setNotice(String(e));
      });
    }, PREVIEW_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [onStrip, palette, previewSeq]);
  const off = () => {
    const l = lastOn.current;
    if (!l) return;
    lastOn.current = null;
    api.previewPalette(false, l.palette, l.seq).catch(() => {});
  };
  useEffect(() => {
    if (!onStrip) return off();
  }, [onStrip]);
  useEffect(() => {
    const leave = () => {
      setOnStrip(false);
      off();
    };
    window.addEventListener("blur", leave);
    window.addEventListener("beforeunload", off);
    return () => {
      window.removeEventListener("blur", leave);
      window.removeEventListener("beforeunload", off);
      off();
    };
  }, []);

  if (!lib) return <p class="text-dim">Loading…</p>;
  const select = (id: number) => {
    setSel(id);
    setDraft(null);
    setHexDraft(null);
    setHexBad(false);
    setNotice("");
  };
  const chooseRole = (r: RoleKey) => {
    setRole(r);
    setHexDraft(null);
    setHexBad(false);
  };
  const colour = palette?.[role];
  const gesture = (what: string) => `${what}-${pickedId}-${role}`;
  const setColour = (c: Hsv, what?: string) =>
    pickedId !== null && setPalette(pickedId, (p) => ({ ...p, [role]: c }), what && gesture(what));

  const matches = (t: string) => t.toLowerCase().includes(q.toLowerCase());
  const target = (r: RoleKey) => drag !== null && !(drag.kind === "role" && drag.role === r);
  const dropOn = (r: RoleKey, e: DragEvent) => {
    e.preventDefault();
    setDrag(null);
    if (pickedId === null) return;
    const roleFrom = e.dataTransfer?.getData(ROLE_TYPE) as RoleKey | "";
    const colourFrom = e.dataTransfer?.getData(COLOUR_TYPE);
    if (roleFrom && roleFrom !== r) {
      setPalette(pickedId, (p) => ({ ...p, [r]: p[roleFrom], [roleFrom]: p[r] }));
    } else if (colourFrom) {
      const [id, from] = colourFrom.split(":") as [string, RoleKey];
      const src = lib.palettes.find(([i]) => i === Number(id))?.[1]?.[from];
      if (src) setPalette(pickedId, (p) => ({ ...p, [r]: { ...src } }));
    }
  };
  const itemClass = (on: boolean) =>
    `flex min-w-0 flex-1 items-center gap-2 rounded-lg px-2 py-1.5 text-left text-body ${on ? "bg-card text-accent outline outline-1 outline-accent" : ""}`;

  return (
    <div class="-m-6 flex min-h-[calc(100%+3rem)]">
      <nav aria-label="Palettes" class="w-52 shrink-0 space-y-2 border-r border-border p-3">
        <input
          type="search"
          aria-label="Search palettes"
          {...help(H.search("Palettes"))}
          placeholder="Search"
          value={q}
          onInput={(e) => setQ(e.currentTarget.value)}
          class="w-full rounded-card border border-border bg-card px-3 py-2"
        />
        <h3 class="mb-1 mt-3 t-label">Palettes</h3>
        <ul>
          {lib.palettes
            .filter(([, p]) => matches(p.name))
            .map(([id, p]) => (
              <li key={id} data-palette={id} class="flex items-center gap-1">
                <span data-grip aria-hidden="true" class="text-[#555b63] before:content-['⋮⋮']" />
                <span class="inline-flex gap-px">
                  {ROLES.map(([r]) => (
                    <i
                      key={r}
                      data-swatch={r}
                      draggable
                      {...help(H.swatch(p.name, r))}
                      onDragStart={(e) => {
                        e.dataTransfer?.setData(COLOUR_TYPE, `${id}:${r}`);
                        setDrag({ kind: "colour" });
                      }}
                      onDragEnd={() => setDrag(null)}
                      class="block h-3 w-3 cursor-grab rounded-sm"
                      style={{ background: css(p[r]) }}
                    />
                  ))}
                </span>
                <button
                  type="button"
                  aria-current={id === pickedId ? "true" : undefined}
                  {...help(H.openPalette)}
                  onClick={() => select(id)}
                  class={itemClass(id === pickedId)}
                >
                  <span class="truncate">{p.name}</span>
                </button>
              </li>
            ))}
        </ul>
        <button
          type="button"
          disabled={lib.palettes.length >= MAX_PALETTES}
          {...help(H.newPalette)}
          onClick={() => add(pastelPalette(Math.floor(Math.random() * 360)))}
          class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-body text-dim disabled:opacity-40"
        >
          <Icon name="plus" />
          New palette
        </button>
      </nav>
      <div class="min-w-0 flex-1 space-y-3 p-4">
        {palette && colour && pickedId !== null ? (
          <>
            <div class="flex items-center gap-2">
              <label for="pal-name" class="t-label">
                Name
              </label>
              <input
                id="pal-name"
                {...help(H.paletteName)}
                value={draft ?? palette.name}
                onInput={(e) => {
                  const name = e.currentTarget.value;
                  if (!nameOk(name)) return setDraft(name);
                  setDraft(null);
                  setPalette(pickedId, (p) => ({ ...p, name }), `name-${pickedId}`);
                }}
                class="rounded-md border border-border bg-bg px-2 py-1"
              />
            </div>
            {draft !== null && <p class="text-red">{NAME_MSG}</p>}
            {notice && (
              <p data-notice class="text-red">
                {notice}
              </p>
            )}
            <div class="grid grid-cols-3 gap-3">
              {ROLES.map(([r, title, sub]) => (
                <button
                  type="button"
                  key={r}
                  data-role={r}
                  {...help(H.roleCard(title))}
                  draggable
                  aria-pressed={r === role}
                  onClick={() => chooseRole(r)}
                  onDragStart={(e) => {
                    e.dataTransfer?.setData(ROLE_TYPE, r);
                    setDrag({ kind: "role", role: r });
                  }}
                  onDragEnd={() => setDrag(null)}
                  onDragOver={(e) => target(r) && e.preventDefault()}
                  onDrop={(e) => dropOn(r, e)}
                  class={`cursor-grab rounded-card border bg-card p-2 text-left ${r === role ? "border-accent" : "border-border"} ${target(r) ? "outline outline-2 outline-accent" : ""}`}
                >
                  <span
                    class="mb-1.5 block h-14 rounded-md"
                    style={{ background: css(palette[r]) }}
                  />
                  <b class="t-head block">{title}</b>
                  <small class="block text-small text-dim">{sub}</small>
                </button>
              ))}
            </div>
            <div class="flex items-start gap-5 rounded-card border border-border bg-card p-3">
              <ColourWheel value={colour} onChange={(c) => setColour(c, "wheel")} />
              <div class="min-w-0 flex-1 space-y-2">
                <div class="t-label">Editing: {ROLES.find(([r]) => r === role)?.[1]}</div>
                <div class="flex items-center gap-3">
                  <span
                    data-big-swatch
                    class="h-11 w-11 shrink-0 rounded-md"
                    style={{ background: css(colour) }}
                  />
                  <input
                    aria-label="Hex colour"
                    {...help(H.hex)}
                    value={hexDraft ?? toHex(colour)}
                    onInput={(e) => {
                      const text = e.currentTarget.value;
                      setHexDraft(text);
                      const c = fromHex(text);
                      setHexBad(!c);
                      if (c) setColour(c, "hex");
                    }}
                    onFocusOut={() => {
                      setHexDraft(null);
                      setHexBad(false);
                    }}
                    class="w-28 rounded-md border border-border bg-bg px-2 py-1 font-mono"
                  />
                </div>
                {hexBad && (
                  <p role="alert" class="text-red">
                    {HEX_MSG}
                  </p>
                )}
                <div data-brightness class="flex items-center gap-2 t-label">
                  <label for="pal-bright">Brightness</label>
                  <span>· {Math.round(colour.v / 10)} %</span>
                </div>
                <input
                  id="pal-bright"
                  type="range"
                  min={0}
                  max={1000}
                  aria-label="Brightness"
                  {...help(H.brightness)}
                  value={colour.v}
                  onInput={(e) =>
                    setColour({ ...colour, v: Number(e.currentTarget.value) }, "bright")
                  }
                  class="v-slider w-full"
                  style={{
                    background: `linear-gradient(90deg,#000,${css({ ...colour, v: 1000 })})`,
                  }}
                />
                <div class="t-label">Quick colours</div>
                <div class="flex flex-wrap gap-1.5">
                  {QUICK.map((hex) => (
                    <button
                      type="button"
                      key={hex}
                      aria-label={`Quick colour ${hex.toUpperCase()}`}
                      {...help(H.quickColour)}
                      onClick={() => {
                        const c = fromHex(hex);
                        if (c) setColour(c);
                      }}
                      class="h-5 w-5 rounded border border-white/10"
                      style={{ background: hex }}
                    />
                  ))}
                </div>
              </div>
            </div>
            {previewEntry && previewSeq !== null && (
              <Simulation sequence={previewEntry[1]} pieces={lib.shape.pieces} palette={palette}>
                <label class="flex items-center gap-2 text-body text-dim">
                  Sequence:
                  <select
                    aria-label="Sequence"
                    {...help(H.paletteSequence)}
                    value={previewSeq}
                    onChange={(e) => setSeqId(Number(e.currentTarget.value))}
                    class="rounded-md border border-border bg-bg px-2 py-1 text-text"
                  >
                    {sequences.map(([id, s]) => (
                      <option key={id} value={id}>
                        {s.name}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  aria-label="Show on strip"
                  aria-pressed={onStrip}
                  onClick={() => setOnStrip((v) => !v)}
                  class={`flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-body ${onStrip ? "border-accent text-accent" : "border-border text-dim"}`}
                  {...help(H.showOnStrip)}
                >
                  <Icon name="strip" />
                  Show on strip
                </button>
              </Simulation>
            )}
            <div data-used-by class="flex flex-wrap items-center gap-2 text-dim">
              {users.length ? (
                <>
                  Used by:
                  {users.map((s) => (
                    <button
                      type="button"
                      key={s.id}
                      onClick={() => onGoSets?.()}
                      {...help(H.usedBy)}
                      class="rounded-lg border border-border bg-card px-2 py-0.5 text-text"
                    >
                      {s.name}
                    </button>
                  ))}
                </>
              ) : (
                "No set uses this palette."
              )}
            </div>
          </>
        ) : (
          <p class="text-dim">No palettes yet. Use New palette.</p>
        )}
      </div>
    </div>
  );
}
