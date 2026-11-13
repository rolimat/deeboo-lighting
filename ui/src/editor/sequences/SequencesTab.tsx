import { DRAG_TYPE } from "../drag";
import { useEffect, useRef, useState } from "preact/hooks";
import { api, type Library, type PatternName, type Sequence } from "../api";
import type { MomentName } from "../../api";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import type { useLibrary } from "../state/library";
import type { TopBarProps } from "../shell/TopBar";
import { ArrangementGrid, ROLE_NAMES } from "./ArrangementGrid";
import { ClipBar } from "./ClipBar";
import { BAR, MAX_BARS, doubleBars, fromCells, halveBars, setBars, toCells } from "./cells";
import { help } from "../shell/Help";
import { H } from "../help-texts";
import { css, roleColour } from "./RolePicker";
import { Simulation } from "./Simulation";

export type BarActions = Pick<
  TopBarProps,
  "onDuplicate" | "onDelete" | "deleteLabel" | "deleteGuard"
>;
type Store = ReturnType<typeof useLibrary>;

const MAX_SEQUENCES = 64;
const NAME_MSG = "Name must be 1 to 63 characters, without control characters.";
const SEED = 1; // generated previews are the same for the whole session
const ZOOMS = [1, 2, 4];
const PAINT = ["principal", "contraste", "suave", "off"] as const;

const MOMENTS: [MomentName, "rhythm" | "calm" | "buildup", IconName, string][] = [
  ["rhythm", "rhythm", "rhythm", "Rhythm"],
  ["calm", "calm", "calm", "Calm"],
  ["buildUp", "buildup", "buildup", "Build-up"],
];
export const PATTERNS: Record<MomentName, [PatternName, string][]> = {
  rhythm: [
    ["halves", "Halves"],
    ["endsCentre", "Ends ↔ centre"],
    ["oddEven", "Odd / even"],
    ["quarters", "Quarters"],
    ["snake", "Snake"],
    ["mirror", "Mirror"],
  ],
  calm: [
    ["drift", "Drift"],
    ["sparse", "Sparse"],
  ],
  buildUp: [
    ["groupFlash", "Group flash"],
    ["fastSnake", "Fast snake"],
    ["strobe", "Strobe"],
  ],
};

export type Sel =
  { k: "seq"; id: number } | { k: "gen"; moment: MomentName; pattern: PatternName | null };

const nameOk = (n: string) => {
  const len = [...n].length;
  // eslint-disable-next-line no-control-regex
  return len >= 1 && len <= 63 && !/[\u0000-\u001f\u007f-\u009f]/.test(n);
};
const tracksOf = (set: Library["sets"][number]) =>
  ({ rhythm: set.rhythm, calm: set.calm, buildUp: set.buildup }) as const;
const hasId = (refs: Library["sets"][number]["rhythm"]["sequences"], id: number) =>
  refs.some((r) => typeof r === "object" && "id" in r && r.id === id);
const momentInfo = (m: string) =>
  MOMENTS.find(([v]) => v === m) ?? (MOMENTS[0] as (typeof MOMENTS)[number]);

function Dialog(p: { text: string; ok: string; onOk: () => void; onCancel: () => void }) {
  return (
    <div class="fixed inset-0 z-10 grid place-items-center bg-black/50">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Confirm"
        class="rounded-card border border-border bg-card p-5"
      >
        <p class="mb-4">{p.text}</p>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            onClick={p.onCancel}
            {...help(H.cancelDialog)}
            class="rounded-card border border-border px-4 py-2"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={p.onOk}
            {...help(H.changeMoment)}
            class="rounded-card bg-accent px-4 py-2 text-bg"
          >
            {p.ok}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Sequences tab: browser by moment, and an Arrangement-style editor for one sequence. */
export function SequencesTab({
  store,
  onActions,
  stripSegments,
  initial,
}: {
  store: Store;
  /** The top bar's Duplicate/Delete for the selection (empty when nothing can be acted on). */
  onActions: (a: BarActions | null) => void;
  stripSegments?: number;
  /** Opened from a clip in Sets. */
  initial?: Sel | null;
}) {
  const { lib, edit } = store;
  const [sel, setSel] = useState<Sel | null>(initial ?? null);
  useEffect(
    () =>
      store.onRemap((m) =>
        setSel((s) => (s?.k === "seq" ? { k: "seq", id: m.sequences.get(s.id) ?? s.id } : s)),
      ),
    [store.onRemap],
  );
  const [previewId, setPreviewId] = useState<number | null>(null); // UI only, never saved
  useEffect(
    () => store.onRemap((m) => setPreviewId((p) => (p === null ? p : (m.palettes.get(p) ?? p)))),
    [store.onRemap],
  );
  const [role, setRole] = useState<string>("principal");
  const [zoom, setZoom] = useState(1);
  const [q, setQ] = useState("");
  const [draft, setDraft] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const [pending, setPending] = useState<{ moment: MomentName; text: string } | null>(null);
  const [gen, setGen] = useState<{ key: string; seq: Sequence } | null>(null);
  const [counts, setCounts] = useState<number[]>([]);
  const [half, setHalf] = useState<number | null>(null);
  const gesture = useRef(0);
  const latest = useRef<{ lib?: Library; id?: number }>({}); // the drag handler outlives renders

  const picked: Sel | null =
    sel?.k === "seq" && !lib?.sequences.some(([id]) => id === sel.id)
      ? null
      : (sel ?? (lib?.sequences[0] ? { k: "seq", id: lib.sequences[0][0] } : null));
  const entry = picked?.k === "seq" ? lib?.sequences.find(([id]) => id === picked.id) : undefined;
  const genKey = picked?.k === "gen" ? `${picked.moment}/${picked.pattern ?? "any"}` : "";
  const shown = entry?.[1] ?? (gen?.key === genKey ? gen.seq : undefined);
  const readOnly = picked?.k === "gen";

  // Generated entries: a fixed-seed preview from Rust.
  useEffect(() => {
    if (picked?.k !== "gen") return;
    let alive = true;
    const p = picked.pattern
      ? api.patternSequence(picked.pattern, SEED)
      : api.previewGenerated(picked.moment);
    p.then(
      (seq) => alive && setGen({ key: genKey, seq }),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [genKey]);

  // Debounced: one call after a burst of edits (a drag paints many cells).
  const paletteId = (() => {
    if (!lib) return 0;
    if (previewId !== null && lib.palettes.some(([id]) => id === previewId)) return previewId;
    const first = lib.sets.find((s) => s.id === lib.activeSet)?.palettes[0];
    return lib.palettes.some(([id]) => id === first)
      ? (first as number)
      : (lib.palettes[0]?.[0] ?? 0);
  })();
  const seen = useRef(0);
  useEffect(() => {
    if (!entry) return setCounts([]);
    const ticket = ++seen.current;
    const t = setTimeout(() => {
      api.messageCounts(entry[1], paletteId).then(
        (c) => ticket === seen.current && setCounts(c),
        () => ticket === seen.current && setCounts([]),
      );
    }, 150);
    return () => clearTimeout(t);
  }, [entry?.[1].steps, paletteId]);

  // The top bar's actions read the latest state through this ref, so they can stay stable.
  const live = useRef<{ dup: () => void; del: () => void; guard: () => boolean }>({
    dup: () => {},
    del: () => {},
    guard: () => false,
  });
  const newId = () => Math.max(1_000_000, ...(lib?.sequences.map(([id]) => id) ?? [])) + 1;
  const add = (seq: Sequence) => {
    if ((lib?.sequences.length ?? 0) >= MAX_SEQUENCES) return setNotice("At most 64 sequences.");
    const id = newId();
    edit((l) => ({ ...l, sequences: [...l.sequences, [id, seq]] }));
    setSel({ k: "seq", id });
    setDraft(null);
    setNotice("");
  };
  live.current = {
    dup: () =>
      shown &&
      entry &&
      add({
        ...shown,
        name: [...`${shown.name} copy`].slice(0, 63).join(""),
        steps: shown.steps.map((s) => ({ ...s, roles: [...s.roles] })),
      }),
    guard: () => {
      if (!lib || !entry) return false;
      const id = entry[0];
      const users = lib.sets.filter((set) =>
        Object.values(tracksOf(set)).some((t) => hasId(t.sequences, id)),
      );
      if (!users.length) return false;
      setNotice(`Used by ${users.map((u) => u.name).join(", ")}. Remove it from there first.`);
      return true;
    },
    del: () => {
      if (!lib || !entry || live.current.guard()) return;
      const id = entry[0];
      edit((l) => ({ ...l, sequences: l.sequences.filter(([i]) => i !== id) }));
      setSel(null);
      setNotice("");
    },
  };
  const canAct = !!entry;
  const label = entry?.[1].name;
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

  if (!lib) return <p class="text-dim">Loading…</p>;
  const pieces = lib.shape.pieces.length;
  const palette = lib.palettes.find(([id]) => id === paletteId)?.[1];
  const select = (s: Sel) => {
    setSel(s);
    setDraft(null);
    setNotice("");
    setPending(null);
  };
  const editSeq = (fn: (s: Sequence) => Sequence, coalesce?: string) => {
    if (entry)
      edit(
        (l) => ({
          ...l,
          sequences: l.sequences.map(([id, s]) => (id === entry[0] ? [id, fn(s)] : [id, s])),
        }),
        { coalesce },
      );
  };
  latest.current = { lib, id: entry?.[0] };
  const cols = shown ? toCells(shown.steps) : [];
  const bars = cols.length / BAR;

  const paint = (seg: number, col: number) => {
    // Cells come from the store, not the render: several cells can be entered before a re-render.
    const cur = lib && entry ? lib.sequences.find(([id]) => id === entry[0])?.[1] : undefined;
    if (!cur || readOnly) return;
    const c = toCells(cur.steps);
    if (c[col]?.[seg] === role) return;
    editSeq(
      (s) => ({
        ...s,
        steps: fromCells(
          toCells(s.steps).map((cc, i) =>
            i === col ? cc.map((r, j) => (j === seg ? role : r)) : cc,
          ),
        ),
      }),
      `paint-${gesture.current}`,
    );
  };
  const changeMoment = (m: MomentName) => {
    if (!entry || m === entry[1].moment) return;
    const id = entry[0];
    const track = momentInfo(m)[1];
    const used = lib.sets.flatMap((set) =>
      MOMENTS.filter(([v, key]) => key !== track && hasId(tracksOf(set)[v].sequences, id)).map(
        ([, , , name]) => `${set.name} (${name})`,
      ),
    );
    if (used.length)
      return setPending({
        moment: m,
        text: `Used by ${used.join(", ")}. It will be removed there.`,
      });
    applyMoment(m);
  };
  const applyMoment = (m: MomentName) => {
    if (!entry) return;
    const id = entry[0];
    const track = momentInfo(m)[1];
    edit((l) => ({
      ...l,
      sequences: l.sequences.map(([i, s]) => (i === id ? [i, { ...s, moment: m }] : [i, s])),
      sets: l.sets.map((set) => {
        const strip = (t: typeof set.rhythm, key: string) => {
          if (key === track || !hasId(t.sequences, id)) return t;
          const rest = t.sequences.filter(
            (r) => !(typeof r === "object" && "id" in r && r.id === id),
          );
          return { ...t, sequences: rest.length ? rest : ["generated" as const] }; // a track needs at least one
        };
        return {
          ...set,
          rhythm: strip(set.rhythm, "rhythm"),
          calm: strip(set.calm, "calm"),
          buildup: strip(set.buildup, "buildup"),
        };
      }),
    }));
    setPending(null);
  };

  // columns where a step starts that costs more than one message
  const warnings = new Map<number, number>();
  if (entry) {
    let at = 0;
    entry[1].steps.forEach((s, i) => {
      if ((counts[i] ?? 0) > 1) warnings.set(at, counts[i] as number);
      at += toCells([s]).length;
    });
  }

  const matches = (t: string) => t.toLowerCase().includes(q.toLowerCase());
  const itemClass = (on: boolean) =>
    `flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-body ${on ? "bg-card text-accent outline outline-1 outline-accent" : ""}`;
  const info = shown ? momentInfo(shown.moment) : null;

  return (
    <div class="-m-6 flex min-h-[calc(100%+3rem)]">
      <nav aria-label="Sequences" class="w-52 shrink-0 space-y-2 border-r border-border p-3">
        <input
          type="search"
          aria-label="Search sequences"
          {...help(H.search("Sequences"))}
          placeholder="Search"
          value={q}
          onInput={(e) => setQ(e.currentTarget.value)}
          class="w-full rounded-card border border-border bg-card px-3 py-2"
        />
        {MOMENTS.map(([m, , icon, name]) => (
          <div key={m}>
            <h3 class="mb-1 mt-3 flex items-center gap-1.5 t-label">
              <Icon name={icon} />
              {name}
            </h3>
            <ul>
              {lib.sequences
                .filter(([, s]) => s.moment === m && matches(s.name))
                .map(([id, s]) => (
                  <li key={id}>
                    <button
                      type="button"
                      aria-current={picked?.k === "seq" && picked.id === id ? "true" : undefined}
                      {...help(H.openSequence)}
                      onClick={() => select({ k: "seq", id })}
                      class={itemClass(picked?.k === "seq" && picked.id === id)}
                    >
                      <span
                        data-grip
                        aria-hidden="true"
                        class="text-[#555b63] before:content-['⋮⋮']"
                      />
                      <span data-moment-icon class="text-dim">
                        <Icon name={icon} />
                      </span>
                      <span class="truncate">{s.name}</span>
                    </button>
                  </li>
                ))}
              {[[null, "Any pattern"] as const, ...PATTERNS[m]]
                .filter(([, l]) => matches(l))
                .map(([p, l]) => {
                  const on = picked?.k === "gen" && picked.moment === m && picked.pattern === p;
                  return (
                    <li key={l}>
                      <button
                        type="button"
                        aria-current={on ? "true" : undefined}
                        {...help(H.openPattern)}
                        onClick={() => select({ k: "gen", moment: m, pattern: p })}
                        class={itemClass(on)}
                      >
                        <span
                          data-grip
                          aria-hidden="true"
                          class="text-[#555b63] before:content-['⋮⋮']"
                        />
                        <span data-moment-icon class="text-dim">
                          <Icon name={icon} />
                        </span>
                        <span class="truncate">{l}</span>
                        <span data-generated class="ml-auto text-accent">
                          <Icon name="generated" />
                        </span>
                      </button>
                    </li>
                  );
                })}
            </ul>
          </div>
        ))}
        <button
          type="button"
          disabled={lib.sequences.length >= MAX_SEQUENCES}
          {...help(H.newSequence)}
          onClick={() => {
            const off = Array<string>(pieces).fill("off");
            add({
              name: "New sequence",
              moment: picked?.k === "gen" ? picked.moment : (shown?.moment ?? "rhythm"),
              steps: fromCells(Array.from({ length: BAR }, () => off)),
            });
          }}
          class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-body text-dim disabled:opacity-40"
        >
          <Icon name="plus" />
          New sequence
        </button>
        <h4 class="mt-4 t-label">Palettes (drag onto the grid)</h4>
        <ul>
          {lib.palettes.map(([id, p]) => (
            <li key={id}>
              <button
                type="button"
                draggable
                data-palette={id}
                {...help(H.previewPaletteItem)}
                onDragStart={(e) => e.dataTransfer?.setData(DRAG_TYPE, String(id))}
                onClick={() => setPreviewId(id)}
                class={itemClass(id === paletteId)}
              >
                <span class="inline-flex gap-px">
                  {[p.principal, p.contraste, p.suave].map((c, i) => (
                    <i
                      key={i}
                      class="block h-2.5 w-2.5 rounded-sm"
                      style={{ background: css(c) }}
                    />
                  ))}
                </span>
                <span class="truncate">{p.name}</span>
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div class="min-w-0 flex-1 space-y-3 p-4">
        {stripSegments !== undefined && stripSegments !== pieces && (
          <p class="text-dim">
            These sequences are for {pieces} segments; the strip has {stripSegments}. Save the strip
            shape to adapt them.
          </p>
        )}
        {shown && palette && info ? (
          <>
            <div class="flex flex-wrap items-center gap-x-5 gap-y-2">
              {readOnly ? (
                <span class="t-head">{shown.name}</span>
              ) : (
                <div class="flex items-center gap-2">
                  <label for="seq-name" class="t-label">
                    Name
                  </label>
                  <input
                    id="seq-name"
                    {...help(H.sequenceName)}
                    value={draft ?? shown.name}
                    onInput={(e) => {
                      const name = e.currentTarget.value;
                      if (!nameOk(name)) return setDraft(name);
                      setDraft(null);
                      editSeq((s) => ({ ...s, name }), `name-${entry?.[0]}`);
                    }}
                    class="rounded-md border border-border bg-bg px-2 py-1"
                  />
                </div>
              )}
              {!readOnly && (
                <div class="flex items-center gap-2" role="group" aria-label="Moment">
                  <span class="t-label">Moment</span>
                  <div class="seg">
                    {MOMENTS.map(([m, , icon, name]) => (
                      <button
                        type="button"
                        key={m}
                        aria-label={`Moment ${name}`}
                        {...help(H.moment[m])}
                        aria-pressed={shown.moment === m}
                        onClick={() => changeMoment(m)}
                        class="flex items-center gap-1.5 px-2 py-1 text-body"
                      >
                        <Icon name={icon} />
                        {name}
                      </button>
                    ))}
                  </div>
                </div>
              )}
              <div class="flex items-center gap-2">
                <label for="seq-palette" class="t-label">
                  Preview palette
                </label>
                <select
                  id="seq-palette"
                  {...help(H.previewPalette)}
                  value={paletteId}
                  onChange={(e) => setPreviewId(Number(e.currentTarget.value))}
                  class="rounded-md border border-border bg-bg px-2 py-1"
                >
                  {lib.palettes.map(([id, p]) => (
                    <option key={id} value={id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </div>
              {readOnly && (
                <button
                  type="button"
                  onClick={() =>
                    add({
                      ...shown,
                      name:
                        picked?.k === "gen" && picked.pattern ? shown.name : `Generated ${info[3]}`,
                      moment: picked?.k === "gen" ? picked.moment : shown.moment,
                    })
                  }
                  {...help(H.duplicateToEdit)}
                  class="rounded-md border border-accent px-3 py-1 text-accent"
                >
                  Duplicate to edit
                </button>
              )}
            </div>
            {draft !== null && <p class="text-red">{NAME_MSG}</p>}
            {notice && <p class="text-red">{notice}</p>}
            {readOnly && <p class="text-dim">Generated from your strip. Read only.</p>}
            <Simulation
              sequence={shown}
              pieces={lib.shape.pieces}
              palette={palette}
              onHalf={setHalf}
            />
            <ArrangementGrid
              cols={cols}
              palette={palette}
              zoom={zoom}
              readOnly={readOnly}
              warnings={warnings}
              playhead={half === null || !cols.length ? null : (half % cols.length) / cols.length}
              onPaintStart={() => gesture.current++}
              onPaint={paint}
              onDropPalette={setPreviewId}
              clip={
                <ClipBar
                  name={shown.name}
                  icon={info[2]}
                  bars={bars}
                  readOnly={readOnly}
                  onBars={(n) => {
                    // Against the store, not the render: a drag can reverse before a re-render.
                    const { lib: l, id } = latest.current;
                    const cur = l?.sequences.find(([i]) => i === id)?.[1];
                    if (!cur || toCells(cur.steps).length === n * BAR) return;
                    editSeq(
                      (s) => ({ ...s, steps: fromCells(setBars(toCells(s.steps), n)) }),
                      `bars-${gesture.current}`,
                    );
                  }}
                />
              }
            />
            {!readOnly && (
              <div class="flex items-center gap-2 text-body text-dim">
                Length
                <button
                  type="button"
                  aria-label="Halve length"
                  disabled={bars <= 1}
                  {...help(H.halve)}
                  onClick={() =>
                    editSeq((s) => ({ ...s, steps: fromCells(halveBars(toCells(s.steps))) }))
                  }
                  class="rounded-md border border-border bg-card px-2 py-1 text-text disabled:opacity-40"
                >
                  ÷2
                </button>
                <button
                  type="button"
                  aria-label="Double length"
                  disabled={bars >= MAX_BARS}
                  {...help(H.double)}
                  onClick={() =>
                    editSeq((s) => ({ ...s, steps: fromCells(doubleBars(toCells(s.steps))) }))
                  }
                  class="rounded-md border border-border bg-card px-2 py-1 text-text disabled:opacity-40"
                >
                  ×2
                </button>
              </div>
            )}
            <div class="flex flex-wrap items-center gap-2 text-body text-dim">
              Paint
              {PAINT.map((r) => (
                <button
                  type="button"
                  key={r}
                  aria-label={`Paint ${ROLE_NAMES[r]}`}
                  {...help(H.paintRole(ROLE_NAMES[r] ?? r))}
                  aria-pressed={role === r}
                  onClick={() => setRole(r)}
                  class={`flex items-center gap-1.5 rounded-md border bg-card px-2 py-1 text-text ${role === r ? "border-accent" : "border-border"}`}
                >
                  <i
                    class="block h-2.5 w-2.5 rounded-full border border-border"
                    style={{ background: roleColour(r, palette) }}
                  />
                  {ROLE_NAMES[r]}
                </button>
              ))}
              <span class="ml-auto flex items-center gap-1.5">
                Zoom
                <button
                  type="button"
                  aria-label="Zoom out"
                  {...help(H.zoomOut)}
                  disabled={zoom <= 1}
                  onClick={() => setZoom(ZOOMS[Math.max(0, ZOOMS.indexOf(zoom) - 1)] as number)}
                  class="rounded-md border border-border bg-card p-1 text-text disabled:opacity-40"
                >
                  <Icon name="zoomout" />
                </button>
                <button
                  type="button"
                  aria-label="Zoom in"
                  {...help(H.zoomIn)}
                  disabled={zoom >= 4}
                  onClick={() => setZoom(ZOOMS[Math.min(2, ZOOMS.indexOf(zoom) + 1)] as number)}
                  class="rounded-md border border-border bg-card p-1 text-text disabled:opacity-40"
                >
                  <Icon name="zoomin" />
                </button>
              </span>
            </div>
          </>
        ) : (
          <p class="text-dim">
            {picked?.k === "gen"
              ? "Loading…"
              : lib.palettes.length
                ? "No sequence yet. Add one with New sequence."
                : "No palette to show colours with."}
          </p>
        )}
      </div>
      {pending && (
        <Dialog
          text={pending.text}
          ok="Change moment"
          onOk={() => applyMoment(pending.moment)}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
