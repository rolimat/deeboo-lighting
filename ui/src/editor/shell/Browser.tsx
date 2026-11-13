import { useState } from "preact/hooks";
import { help } from "./Help";
import { H } from "../help-texts";

export interface BrowserItem {
  id: number;
  label: string;
}

/** Left-hand navigator of Sets, Sequences and Palettes: search box and a selectable list. */
export function Browser({
  title,
  items,
  selected,
  onSelect,
}: {
  title: string;
  items: BrowserItem[];
  selected: number | null;
  onSelect: (id: number) => void;
}) {
  const [q, setQ] = useState("");
  const shown = items.filter((i) => i.label.toLowerCase().includes(q.toLowerCase()));
  return (
    <nav aria-label={title} class="w-52 shrink-0 space-y-2 border-r border-border p-3">
      <input
        type="search"
        aria-label={`Search ${title.toLowerCase()}`}
        placeholder="Search"
        {...help(H.search(title))}
        value={q}
        onInput={(e) => setQ(e.currentTarget.value)}
        class="w-full rounded-card border border-border bg-card px-3 py-2"
      />
      <ul class="space-y-1">
        {shown.map((i) => (
          <li key={i.id}>
            <button
              type="button"
              aria-current={i.id === selected ? "true" : undefined}
              {...help(H.browserItem(title))}
              onClick={() => onSelect(i.id)}
              class={`block w-full rounded-card px-3 py-2 text-left ${i.id === selected ? "bg-card text-accent" : ""}`}
            >
              {i.label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
