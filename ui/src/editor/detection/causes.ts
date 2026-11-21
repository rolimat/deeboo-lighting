import type { Cause } from "../api";

/** What the detector's reason ids mean, in words (debug view). */
export const CAUSE_WORDS: Record<Cause, string> = {
  riser: "highs climbed",
  approach: "slow build",
  quiet: "kick gone",
  teaser: "short tease",
  groove: "groove build-up",
  kickDrop: "kick back",
  hatsDrop: "hats back",
  slowReturn: "slow return",
  collapse: "build-up faded",
  timeout: "too long",
  softBreak: "soft break",
  phraseStart: "phrase start",
  undo: "undone",
};
