import { useEffect, useRef, useState } from "preact/hooks";
import type { MomentName, Status } from "../../api";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import {
  api,
  type Interval,
  type LibrarySet,
  type MomentSet,
  type Order,
  type SeqRef,
} from "../api";
import { PATTERNS, type BarActions, type Sel } from "../sequences/SequencesTab";
import type { useLibrary } from "../state/library";
import { canDrop, moveRef, type DragItem } from "./dnd";
import { PaletteRow, Swatch } from "./PaletteRow";
import { Track } from "./Track";

type Store = ReturnType<typeof useLibrary>;
type Field = "rhythm" | "calm" | "buildup";
type Drag = { item: DragItem; name: string } | null;

const MAX_SETS = 16;
const NAME_MSG = "Name must be 1 to 63 characters, without control characters.";
const MOMENTS: [MomentName, Field, IconName, string][] = [
  ["rhythm", "rhythm", "rhythm", "Rhythm"],
  ["calm", "calm", "calm", "Calm"],
  ["buildUp", "buildup", "buildup", "Build-up"],
];
const nameOk = (n: string) => {
  const len = [...n].length;
  // eslint-disable-next-line no-control-regex
  return len >= 1 && len <= 63 && !/[\u0000-\u001f\u007f-\u009f]/.test(n);
};
const anyPattern = (interval: Interval): MomentSet => ({
  sequences: ["generated"],
  order: "inOrder",
  interval,
});

/** Sets tab: browser (sets, sequences, palettes) and the open set as three tracks plus a palette row. */
export function SetsTab({
  store,
  onActions,
  onOpenSequence,
}: {
  store: Store;
  onActions: (a: BarActions | null) => void;
  /** A clip was double-clicked: open it in Sequences. */
  onOpenSequence?: (sel: Sel) => void;
}) {
  const { lib, edit } = store;
  const [sel, setSel] = useState<number | null>(null);
  const [q, setQ] = useState("");
  const [draft, setDraft] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const [drag, setDrag] = useState<Drag>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const held = useRef<(() => void) | null>(null);

  const set =
    lib?.sets.find((s) => s.id === sel) ??
    lib?.sets.find((s) => s.id === store.liveSet) ??
    lib?.sets[0];
  const isLive = !!set && set.id === store.liveSet;
  const sequences = lib?.sequences ?? [];
  const palettes = lib?.palettes ?? [];
  const firstPalette = palettes.find(([id]) => id === set?.palettes[0])?.[1];

  const patch = (fn: (s: LibrarySet) => LibrarySet, coalesce?: string) => {
    if (!set) return;
    edit((l) => ({ ...l, sets: l.sets.map((s) => (s.id === set.id ? fn(s) : s)) }), { coalesce });
    setNotice("");
  };
  const patchMoment = (f: Field, fn: (m: MomentSet) => MomentSet) =>
    patch((s) => ({ ...s, [f]: fn(s[f]) }));

  const live = useRef<{ dup: () => void; del: () => void; guard: () => boolean }>({
    dup: () => {},
    del: () => {},
    guard: () => false,
  });
  live.current = {
    dup: () => {
      if (!set || !lib) return;
      if (lib.sets.length >= MAX_SETS) return setNotice("At most 16 sets.");
      const id = Math.max(1_000_000, ...lib.sets.map((s) => s.id)) + 1;
      const name = [...`${set.name} copy`].slice(0, 63).join("");
      edit((l) => ({ ...l, sets: [...l.sets, { ...structuredClone(set), id, name }] }));
      setSel(id);
    },
    guard: () => {
      if ((lib?.sets.length ?? 0) > 1) return false;
      setNotice("You need at least one set.");
      return true;
    },
    del: () => {
      if (!set) return;
      edit((l) => ({ ...l, sets: l.sets.filter((s) => s.id !== set.id) }));
      setSel(null);
    },
  };
  const label = set?.name;
  useEffect(() => {
    onActions(
      label === undefined
        ? {}
        : {
            onDuplicate: () => live.current.dup(),
            onDelete: () => live.current.del(),
            deleteGuard: () => live.current.guard(),
            deleteLabel: label,
          },
    );
  }, [label === undefined, label]);
  useEffect(() => () => onActions(null), []);

  // Live readout: the moment in force and the palette playing (no polling).
  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    let pushed = false;
    api
      .onStatus((st) => {
        pushed = true;
        setStatus(st);
      })
      .then(
        (u) => (alive ? (off = u) : u()),
        () => {},
      );
    api.getFullStatus().then(
      (st) => alive && !pushed && setStatus(st),
      () => {},
    );
    return () => {
      alive = false;
      off?.();
    };
  }, []);

  // Hold-to-preview: off on release, leaving the button, window blur and unmount.
  const stop = () => {
    held.current?.();
    held.current = null;
  };
  const start = (ref: SeqRef, m: MomentName, on: boolean) => {
    if (!on) return stop();
    if (!firstPalette || held.current) return;
    const p = firstPalette;
    held.current = () => void api.previewSequence(false, p, ref, m).catch(() => {});
    api.previewSequence(true, p, ref, m).catch((e: unknown) => {
      held.current = null;
      setNotice(String(e));
    });
  };
  useEffect(() => {
    window.addEventListener("pointerup", stop);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("blur", stop);
      stop();
    };
  }, []);

  const playLive = (id: number) =>
    api
      .setLiveSet(id)
      .then(() => store.syncLive())
      .catch((e: unknown) => setNotice(String(e)));

  const addSet = () => {
    if (!lib) return;
    if (lib.sets.length >= MAX_SETS) return setNotice("At most 16 sets.");
    const id = Math.max(1_000_000, ...lib.sets.map((s) => s.id)) + 1;
    edit((l) => ({
      ...l,
      sets: [
        ...l.sets,
        {
          id,
          name: "New set",
          rhythm: anyPattern({ bars: 8 }),
          calm: anyPattern("oncePerBreak"),
          buildup: anyPattern({ bars: 2 }),
          palettes: l.palettes.map(([i]) => i),
          paletteOrder: "inOrder",
          paletteChange: "atDrop",
        },
      ],
    }));
    setSel(id);
  };

  const dropOn = (f: Field, m: MomentName, index: number) => {
    const d = drag;
    setDrag(null);
    if (!d || !canDrop(d.item, { kind: "track", moment: m })) return;
    if (d.item.kind === "sequence") {
      const ref = d.item.ref;
      patchMoment(f, (t) => ({
        ...t,
        sequences: [...t.sequences.slice(0, index), ref, ...t.sequences.slice(index)],
      }));
    } else if (d.item.kind === "clip") {
      const from = d.item.index;
      patchMoment(f, (t) => ({ ...t, sequences: moveRef(t.sequences, from, index) }));
    }
  };
  const dropPalette = () => {
    const d = drag;
    setDrag(null);
    if (d?.item.kind !== "palette" || !canDrop(d.item, { kind: "palettes" })) return;
    const id = d.item.id;
    patch((s) => (s.palettes.includes(id) ? s : { ...s, palettes: [...s.palettes, id] }));
  };
  const open = (ref: SeqRef, m: MomentName) =>
    onOpenSequence?.(
      ref === "generated"
        ? { k: "gen", moment: m, pattern: null }
        : "pattern" in ref
          ? { k: "gen", moment: m, pattern: ref.pattern }
          : { k: "seq", id: ref.id },
    );

  if (!lib || !set) return <p class="text-dim">Loading…</p>;
  const match = (t: string) => t.toLowerCase().includes(q.toLowerCase());
  const dragProps = (item: DragItem, name: string) => ({
    draggable: true as const,
    onDragStart: (e: DragEvent) => {
      e.dataTransfer?.setData("text/plain", name);
      setDrag({ item, name });
    },
    onDragEnd: () => setDrag(null),
  });
  const grip = <span data-grip aria-hidden="true" class="text-[#555b63] before:content-['⋮⋮']" />;
  const item =
    "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm cursor-grab";
  const playingMoment = isLive && status?.playing ? status.moment : null;
  const playingPalette = isLive && status?.playing ? status.selection.palette.name : null;

  return (
    <div class="-m-6 flex min-h-[calc(100%+3rem)]">
      <nav aria-label="Sets" class="w-52 shrink-0 space-y-1 border-r border-border p-3">
        <input
          type="search"
          aria-label="Search sets"
          placeholder="Search"
          value={q}
          onInput={(e) => setQ(e.currentTarget.value)}
          class="mb-2 w-full rounded-card border border-border bg-card px-3 py-2"
        />
        <h3 class="text-[10px] uppercase tracking-wider text-dim">Sets</h3>
        <ul>
          {lib.sets
            .filter((s) => match(s.name))
            .map((s) => {
              const playing = s.id === store.liveSet;
              return (
                <li key={s.id} class="flex items-center gap-1">
                  <button
                    type="button"
                    aria-label={playing ? `Playing ${s.name} live` : `Play ${s.name} live`}
                    title={playing ? "Playing live" : "Play live"}
                    onClick={() => !playing && void playLive(s.id)}
                    class={`grid place-items-center ${playing ? "text-accent [&_svg]:fill-current" : "text-dim"}`}
                  >
                    <Icon name="play" />
                  </button>
                  <button
                    type="button"
                    aria-current={s.id === set.id ? "true" : undefined}
                    onClick={() => setSel(s.id)}
                    class={`min-w-0 flex-1 truncate rounded-lg px-2 py-1.5 text-left text-sm ${s.id === set.id ? "bg-card text-accent outline outline-1 outline-accent" : ""}`}
                  >
                    {s.name}
                  </button>
                </li>
              );
            })}
        </ul>
        <button
          type="button"
          onClick={addSet}
          class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm text-dim"
        >
          <Icon name="plus" />
          New set
        </button>
        <h3 class="pt-3 text-[10px] uppercase tracking-wider text-dim">Sequences</h3>
        {MOMENTS.map(([m, , icon, name]) => (
          <div key={m}>
            <h4 class="mb-1 mt-2 flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-dim">
              <Icon name={icon} />
              {name}
            </h4>
            {sequences
              .filter(([, s]) => s.moment === m && match(s.name))
              .map(([id, s]) => (
                <div
                  key={id}
                  data-drag-seq={id}
                  class={item}
                  {...dragProps({ kind: "sequence", ref: { id }, moment: m }, s.name)}
                >
                  {grip}
                  <span class="text-dim">
                    <Icon name={icon} />
                  </span>
                  <span class="truncate">{s.name}</span>
                </div>
              ))}
            {[["any", "Any pattern"] as const, ...PATTERNS[m]]
              .filter(([, l]) => match(l))
              .map(([p, l]) => (
                <div
                  key={p}
                  data-drag-pattern={p}
                  title="Generated from your strip"
                  class={item}
                  {...dragProps(
                    {
                      kind: "sequence",
                      ref: p === "any" ? "generated" : { pattern: p },
                      moment: m,
                    },
                    l,
                  )}
                >
                  {grip}
                  <span class="text-dim">
                    <Icon name={icon} />
                  </span>
                  <span class="truncate">{l}</span>
                  <span class="ml-auto text-accent">
                    <Icon name="generated" />
                  </span>
                </div>
              ))}
          </div>
        ))}
        <h3 class="pt-3 text-[10px] uppercase tracking-wider text-dim">Palettes</h3>
        {palettes
          .filter(([, p]) => match(p.name))
          .map(([id, p]) => (
            <div
              key={id}
              data-drag-palette={id}
              class={item}
              {...dragProps({ kind: "palette", id }, p.name)}
            >
              {grip}
              <Swatch p={p} />
              <span class="truncate">{p.name}</span>
            </div>
          ))}
      </nav>
      <div class="flex min-w-0 flex-1 flex-col gap-3 p-4">
        <div class="flex flex-wrap items-center gap-3">
          <label for="set-name" class="text-[10px] uppercase tracking-wider text-dim">
            Set name
          </label>
          <input
            id="set-name"
            aria-label="Set name"
            value={draft ?? set.name}
            onInput={(e) => {
              const v = e.currentTarget.value;
              setDraft(v);
              if (!nameOk(v)) return setNotice(NAME_MSG);
              patch((s) => ({ ...s, name: v }), `set-name-${set.id}`);
            }}
            onBlur={() => {
              setDraft(null);
              setNotice("");
            }}
            class="min-w-32 rounded-card border border-border bg-bg px-2 py-1"
          />
          <span class="flex-1" />
          <span data-live-state class="flex items-center gap-2 text-sm">
            {isLive ? (
              <span class="flex items-center gap-1.5 text-accent">
                <Icon name="play" />
                Playing live
              </span>
            ) : (
              <>
                <span class="text-dim">Not playing ·</span>
                <button
                  type="button"
                  aria-label="Play this set live"
                  onClick={() => void playLive(set.id)}
                  class="flex items-center gap-1.5 rounded-lg border border-border bg-card px-2 py-1"
                >
                  <Icon name="play" />
                  Play live
                </button>
              </>
            )}
          </span>
        </div>
        {notice && (
          <p role="alert" class="text-sm text-red">
            {notice}
          </p>
        )}
        <div class="grid grid-cols-3 gap-3">
          {MOMENTS.map(([m, f, icon, name]) => (
            <Track
              key={f}
              moment={m}
              field={f}
              label={name}
              icon={icon}
              data={set[f]}
              palette={firstPalette}
              sequences={sequences}
              drag={drag}
              playing={playingMoment === m}
              onDragClip={(index) =>
                setDrag({
                  item: { kind: "clip", from: m, index },
                  name: String(set[f].sequences[index] ?? ""),
                })
              }
              onDragEnd={() => setDrag(null)}
              onDropAt={(i) => dropOn(f, m, i)}
              onRemove={(i) =>
                set[f].sequences.length <= 1
                  ? setNotice("A track needs at least one sequence.")
                  : patchMoment(f, (t) => ({
                      ...t,
                      sequences: t.sequences.filter((_, k) => k !== i),
                    }))
              }
              onOrder={(order: Order) => patchMoment(f, (t) => ({ ...t, order }))}
              onInterval={(v) =>
                patchMoment(f, (t) => ({
                  ...t,
                  interval: v === "oncePerBreak" ? "oncePerBreak" : { bars: Number(v) },
                }))
              }
              onPreview={(ref, on) => start(ref, m, on)}
              onOpen={(ref) => open(ref, m)}
            />
          ))}
        </div>
        <PaletteRow
          set={set}
          palettes={palettes}
          playingName={playingPalette}
          drag={drag}
          onDrop={dropPalette}
          onRemove={(id) =>
            set.palettes.length <= 1
              ? setNotice("A set needs at least one palette.")
              : patch((s) => ({ ...s, palettes: s.palettes.filter((p) => p !== id) }))
          }
          onOrder={(paletteOrder) => patch((s) => ({ ...s, paletteOrder }))}
          onChange={(paletteChange) => patch((s) => ({ ...s, paletteChange }))}
        />
      </div>
    </div>
  );
}
