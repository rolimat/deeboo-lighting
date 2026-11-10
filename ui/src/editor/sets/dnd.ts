import type { MomentName } from "../../api";
import type { SeqRef } from "../api";

/** What is being dragged in the Sets tab. */
export type DragItem =
  | { kind: "sequence"; ref: SeqRef; moment: MomentName }
  | { kind: "palette"; id: number }
  | { kind: "clip"; from: MomentName; index: number };

export type DropTarget = { kind: "track"; moment: MomentName } | { kind: "palettes" };

/** The one rule for every drop zone: a sequence or clip only goes to the track of its own moment. */
export function canDrop(item: DragItem, target: DropTarget): boolean {
  switch (item.kind) {
    case "sequence":
      return target.kind === "track" && target.moment === item.moment;
    case "clip":
      return target.kind === "track" && target.moment === item.from;
    case "palette":
      return target.kind === "palettes";
  }
}

/** Moves `list[from]` to sit before the item that was at `to` (`to === list.length` = the end). */
export function moveRef<T>(list: T[], from: number, to: number): T[] {
  const out = [...list];
  const [item] = out.splice(from, 1);
  out.splice(to > from ? to - 1 : to, 0, item as T);
  return out;
}
