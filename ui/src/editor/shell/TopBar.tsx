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
  /** Runs before the confirmation; returning true means the tab already answered (no dialog). */
  deleteGuard?: () => boolean;
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
      class={`grid h-7 w-7 shrink-0 place-items-center rounded-lg border disabled:opacity-40 ${p.red ? "border-red-dark text-red" : "border-border text-dim"}`}
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
    <header class="flex min-w-0 items-center gap-1.5 border-b border-border px-3 py-2">
      <div role="tablist" aria-label="Sections" class="flex shrink-0 gap-0.5">
        {TABS.map(([id, name]) => (
          <button
            type="button"
            role="tab"
            key={id}
            aria-selected={id === p.tab}
            onClick={() => p.onTab(id)}
            class={`flex items-center gap-1.5 whitespace-nowrap rounded-lg px-2.5 py-1.5 text-sm ${id === p.tab ? "bg-card text-accent" : "text-dim"}`}
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
      {(p.onDuplicate || p.onDelete) && (
        <span class="mx-1 h-5 w-px shrink-0 bg-border" role="separator" />
      )}
      {p.onDuplicate && <IconButton name="duplicate" label="Duplicate" onClick={p.onDuplicate} />}
      {p.onDelete && (
        <IconButton
          name="delete"
          label="Delete"
          red
          onClick={() => !p.deleteGuard?.() && setConfirm(true)}
        />
      )}
      <span
        role="status"
        class={`ml-auto shrink-0 whitespace-nowrap px-2 text-sm ${p.save.state === "error" ? "text-red" : "text-dim"}`}
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
