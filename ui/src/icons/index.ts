import undo from "./undo.svg?raw";
import redo from "./redo.svg?raw";
import duplicate from "./duplicate.svg?raw";
import trash from "./delete.svg?raw";
import help from "./help.svg?raw";
import rhythm from "./rhythm.svg?raw";
import calm from "./calm.svg?raw";
import buildup from "./buildup.svg?raw";
import sets from "./sets.svg?raw";
import sequences from "./sequences.svg?raw";
import palettes from "./palettes.svg?raw";
import strip from "./strip.svg?raw";
import detection from "./detection.svg?raw";
import setup from "./setup.svg?raw";

/** Bundled line icons (24x24, stroke 2); see LICENSE.md. */
export const icons = {
  undo,
  redo,
  duplicate,
  delete: trash,
  help,
  rhythm,
  calm,
  buildup,
  sets,
  sequences,
  palettes,
  strip,
  detection,
  setup,
};
export type IconName = keyof typeof icons;
