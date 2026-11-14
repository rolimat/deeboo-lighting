import type { MomentName } from "../api";
import type { Order } from "./api";
import type { Tab } from "./shell/TopBar";

/**
 * Every hover explanation of the editor, in one place (review and edit here).
 * One sentence each: what the control does. Shown by the tooltip in `shell/Help.tsx`
 * while the "?" toggle is on; controls point at them with `{...help(H.key)}`.
 */
export const H = {
  // Top bar
  tab: {
    sets: "Build sets: which sequences and palettes play in each moment, and in what order.",
    sequences: "Create and edit light sequences, segment by segment.",
    palettes: "Create and edit the three-colour palettes the sequences are painted with.",
    strip: "Connect and describe your LED strip: how many segments and what shape.",
    detection: "Watch the detector live and tune when the show moves to Calm or Build-up.",
    setup: "MIDI clock, audio input, light latency, phrase length and app settings.",
  } satisfies Record<Tab, string>,
  undo: "Undo the last change (⌘Z).",
  redo: "Redo the change you just undid (⇧⌘Z).",
  duplicate: "Make a copy of the selected item.",
  delete: "Delete the selected item. You can undo it.",
  saveState: "Whether your latest changes are saved.",
  helpToggle: "Turn these hover tips on or off.",
  cancelDelete: "Keep it and close this dialog.",
  confirmDelete: "Delete it (⌘Z brings it back).",
  cancelDialog: "Close this dialog without changing anything.",
  changeMoment: "Move the sequence to the new moment; sets that use it drop it from the old track.",
  backToStrip: "Leave the connection steps and return to your connected strip.",

  // Lists (Sets, Sequences, Palettes)
  search: (list: string) => `Filter the ${list.toLowerCase()} by name.`,
  browserItem: (list: string) => `Open this entry of ${list.toLowerCase()}.`,
  openSet: "Open this set to edit it.",
  playLiveRow: "Play this set live on the strip.",
  playingLive: "This set is playing live now.",
  newSet: "Create a new empty set.",
  openSequence: "Open this sequence in the editor.",
  openPattern: "A pattern generated from your strip; open it to see it, then duplicate it to edit.",
  newSequence: "Create a new empty sequence for the selected moment.",
  previewPaletteItem:
    "Show the sequence in this palette; drag it onto the grid to use it for the preview.",
  openPalette: "Open this palette to edit its colours.",
  newPalette: "Create a new palette.",
  swatch: (palette: string, role: string) =>
    `The ${role} colour of ${palette}; drag it onto a role of the open palette to copy it.`,
  dragSequence: "Drag this sequence onto a track of the same moment.",
  dragPattern: "A pattern generated from your strip; drag it onto a track of the same moment.",
  dragPalette: "Drag this palette onto the palette row to add it to the set.",

  // Sets
  setName: "The name of this set.",
  playLive: "Play this set live on the strip.",
  order: {
    inOrder: "In order: goes through the list from first to last, then starts again.",
    random: "Random: picks any entry each time.",
    fixed: "Fixed: stays on the first entry, with no rotation.",
  } satisfies Record<Order, string>,
  changeEvery: (track: string) => `How often the ${track} track switches to its next sequence.`,
  clip: "A sequence in this track: drag to reorder, double-click to open it in Sequences.",
  generated: "Generated from your strip, so it always fits its shape.",
  previewClip: "Hold to preview this sequence on the strip.",
  removeClip: "Remove it from this track.",
  trackZone: "Drop a sequence here to add it to this track.",
  paletteChange: "When the set moves to its next palette.",
  paletteDrop: "Drop a palette here to add it to the set.",
  removePalette: "Remove this palette from the set.",
  playingNow: "This palette is playing now.",

  // Sequences
  sequenceName: "The name of this sequence.",
  moment: {
    rhythm: "Make this a Rhythm sequence, played while the music has its normal groove.",
    calm: "Make this a Calm sequence, played during breaks.",
    buildUp: "Make this a Build-up sequence, played while the music rises to a drop.",
  } satisfies Record<MomentName, string>,
  previewPalette: "The palette used to show this sequence here; it is not saved with it.",
  duplicateToEdit: "Copy this generated sequence into your library so you can edit it.",
  fade: "Fade between steps in Calm; off makes every change a cut.",
  simulation: "Play or pause the preview of this sequence.",
  clipLength: "Drag, or use the arrow keys, to change the length in whole bars.",
  halve: "Halve the length: keeps the first half.",
  double: "Double the length: repeats the sequence once.",
  cell: "Paint this segment at this moment with the chosen colour; click or drag, arrows move around.",
  paintRole: (role: string) => `Paint with ${role}.`,
  zoomOut: "Make the grid cells smaller.",
  zoomIn: "Make the grid cells larger.",

  // Palettes
  paletteName: "The name of this palette.",
  roleCard: (role: string) =>
    `Edit the ${role} colour. Drag it onto another role to swap, or drop a colour from another palette to copy it.`,
  wheel: "Pick hue and saturation: click or drag, or use the arrow keys.",
  hex: "Type the colour as a hex code, such as #2DD4BF.",
  brightness: "How bright this colour is.",
  quickColour: "Use this colour.",
  paletteSequence: "The sequence used to preview this palette.",
  showOnStrip: "Plays the chosen sequence in this palette on the real strip while you edit.",
  usedBy: "Open Sets, where this palette is used.",

  // Strip
  fewerSegments: "Fewer segments are wired to the controller.",
  moreSegments: "More segments are wired to the controller.",
  lightSegment: "Light this segment on the strip, so you can count it.",
  reconnect: "Run the connection steps again for this strip.",
  changeStrip: "Connect a different strip.",
  cancelReduce: "Keep the segments as they are.",
  confirmReduce: "Reduce the segments; sequences lose the extra ones.",
  preset: (label: string) => `Start the shape from "${label}".`,
  piece:
    "One segment of the strip: drag to move it; arrows nudge it; shift + drag moves the whole shape.",
  rotateHandle: "Drag to rotate this segment in 15 degree steps.",
  rotateLeft: "Rotate this segment 15 degrees counter-clockwise.",
  rotateRight: "Rotate this segment 15 degrees clockwise.",
  litOnStrip: "This segment is lit on the real strip.",
  guideClose: "Hide this guide.",
  chooseStrip: "Use this strip.",
  unsupportedStrip: "This device cannot be used as a strip.",
  userCode: "Your Smart Life User Code; it is only used to find your strip.",
  userCodeContinue: "Continue to the QR code.",
  wiredCount: "How many segments actually light up on your strip.",
  saveStrip: "Save the strip with this many segments.",
  wizardCancel: "Stop connecting the strip.",
  wizardRetry: "Try the connection again.",
  drawShape: "Go on to draw the shape of your strip.",

  // Detection
  momentChanges: "How readily the show moves to Calm or Build-up: Steady, Balanced or Reactive.",
  resetDefaults: "Put the sensitivity and the three thresholds back to their defaults.",
  changeInSetup: "Open Setup to change the clock input or the audio device.",
  thresholdKick: "How far the kick must fall below its reference to count as a break.",
  thresholdRiser: "How far the highs must climb above their reference to count as a riser.",
  thresholdHigh: "How far the highs must fall below their reference to help Calm.",

  // Setup
  midiInput: "Pick the app or device that sends the MIDI clock.",
  audioInput: "The input the detector listens to.",
  leadInput: "How much earlier the app sends light, in milliseconds.",
  leadLater: "Send 10 ms later.",
  leadEarlier: "Send 10 ms earlier.",
  calibrate: "Flash the strip on every beat so you can match the light to the kick you hear.",
  phraseBars: (n: number) => `Count ${n} bars as one phrase.`,
  dock: "Choose when the app shows an icon in the Dock; the menu bar icon is always there.",
  quit: "Quit Deeboo Lighting.",
} as const;
