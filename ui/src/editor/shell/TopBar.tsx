import { useState } from "preact/hooks";
import type { SaveState } from "../state/library";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";
import { ConfirmDelete } from "./ConfirmDelete";
import { HelpToggle, useHelp } from "./Help";
import { LiveReadout } from "./LiveReadout";

export const TABS = [
  ["sets", "Sets"],
  ["sequences", "Sequences"],
  ["palettes", "Palettes"],
  ["strip", "Strip"],
  ["detection", "Detection"],
  ["setup", "Setup"],
] as const satisfies readonly (readonly [IconName, string])[];
export type Tab = (typeof TABS)[number][0];

export interface TopBarProps {
  tab: Tab;
  onTab: (t: Tab) => void;
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onDuplicate?: () => void;
  onDelete?: () => void;
  deleteLabel?: string;
  save: SaveState;
  /** Why undo/redo are disabled on this tab (settings apply at once). */
  undoDisabledHint?: string;
}

function IconButton(p: {
  name: IconName;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  hint?: string;
  red?: boolean;
}) {
  const help = useHelp(p.hint ?? p.label);
  return (
    <button
      type="button"
      aria-label={p.label}
      disabled={p.disabled}
      onClick={p.onClick}
      class={`rounded-card p-2 disabled:opacity-40 ${p.red ? "text-red" : ""}`}
      {...help}
    >
      <Icon name={p.name} />
    </button>
  );
}

export function TopBar(p: TopBarProps) {
  const [confirm, setConfirm] = useState(false);
  const noUndo = p.undoDisabledHint;
  return (
    <header class="flex items-center gap-1 border-b border-border px-3 py-2">
      <div role="tablist" aria-label="Sections" class="flex gap-1">
        {TABS.map(([id, name]) => (
          <button
            type="button"
            role="tab"
            key={id}
            aria-selected={id === p.tab}
            onClick={() => p.onTab(id)}
            class={`flex items-center gap-2 rounded-card px-3 py-2 ${id === p.tab ? "bg-card text-accent" : "text-dim"}`}
          >
            <Icon name={id} />
            {name}
          </button>
        ))}
      </div>
      <IconButton
        name="undo"
        label="Undo"
        hint={noUndo ?? "Undo (⌘Z)"}
        disabled={!!noUndo || !p.canUndo}
        onClick={p.onUndo}
      />
      <IconButton
        name="redo"
        label="Redo"
        hint={noUndo ?? "Redo (⇧⌘Z)"}
        disabled={!!noUndo || !p.canRedo}
        onClick={p.onRedo}
      />
      {(p.onDuplicate || p.onDelete) && <span class="mx-1 h-5 w-px bg-border" role="separator" />}
      {p.onDuplicate && <IconButton name="duplicate" label="Duplicate" onClick={p.onDuplicate} />}
      {p.onDelete && (
        <IconButton name="delete" label="Delete" red onClick={() => setConfirm(true)} />
      )}
      <span
        role="status"
        class={`ml-auto px-2 text-sm ${p.save.state === "error" ? "text-red" : "text-dim"}`}
      >
        {p.save.state === "saving"
          ? "Saving…"
          : p.save.state === "saved"
            ? "✓ Saved"
            : `Not saved — ${p.save.reason}`}
      </span>
      <HelpToggle />
      <LiveReadout />
      {confirm && p.onDelete && (
        <ConfirmDelete
          label={p.deleteLabel ?? "this"}
          onCancel={() => setConfirm(false)}
          onConfirm={() => {
            setConfirm(false);
            p.onDelete?.();
          }}
        />
      )}
    </header>
  );
}
