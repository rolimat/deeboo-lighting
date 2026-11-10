import undo from "./undo.svg?raw";
import redo from "./redo.svg?raw";
import duplicate from "./duplicate.svg?raw";
import trash from "./delete.svg?raw";
import help from "./help.svg?raw";
import rhythm from "./rhythm.svg?raw";
import calm from "./calm.svg?raw";
import buildup from "./buildup.svg?raw";
import generated from "./generated.svg?raw";
import play from "./play.svg?raw";
import pause from "./pause.svg?raw";
import zoomin from "./zoomin.svg?raw";
import zoomout from "./zoomout.svg?raw";
import plus from "./plus.svg?raw";
import sets from "./sets.svg?raw";
import sequences from "./sequences.svg?raw";
import palettes from "./palettes.svg?raw";
import strip from "./strip.svg?raw";
import detection from "./detection.svg?raw";
import setup from "./setup.svg?raw";
import inorder from "./inorder.svg?raw";
import random from "./random.svg?raw";
import fixed from "./fixed.svg?raw";
import close from "./close.svg?raw";
import wifi from "./wifi.svg?raw";
import rotateccw from "./rotateccw.svg?raw";
import rotatecw from "./rotatecw.svg?raw";
import lightbulb from "./lightbulb.svg?raw";
import preview from "./preview.svg?raw";
import midi from "./midi.svg?raw";
import mic from "./mic.svg?raw";
import clock from "./clock.svg?raw";
import phrase from "./phrase.svg?raw";
import power from "./power.svg?raw";

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
  generated,
  play,
  pause,
  zoomin,
  zoomout,
  plus,
  sets,
  sequences,
  palettes,
  strip,
  detection,
  setup,
  inorder,
  random,
  fixed,
  close,
  preview,
  wifi,
  rotateccw,
  rotatecw,
  lightbulb,
  midi,
  mic,
  clock,
  phrase,
  power,
};
export type IconName = keyof typeof icons;
