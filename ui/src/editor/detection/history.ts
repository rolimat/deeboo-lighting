import type { DetectorEvent } from "../api";

/** 8 bars of 4 beats: what the timeline shows. */
export const RING_BEATS = 32;

/** The ring after `e` arrives: the last 32 beats, oldest first. */
export const pushBeat = (ring: DetectorEvent[], e: DetectorEvent): DetectorEvent[] =>
  [...ring, e].slice(-RING_BEATS);
