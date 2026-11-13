import { beforeEach, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  api: {
    uiCommand: vi.fn(),
    getSettings: vi.fn(),
    saveSettings: vi.fn(),
    quit: vi.fn(),
    openEditor: vi.fn(),
    selectSet: vi.fn(),
    selectPalette: vi.fn(),
  },
}));
import { api, type Selection, type Settings, type Status } from "./api";
import { mountPopover } from "./view-popover";

const settings: Settings = {
  midiSource: null,
  audioInput: null,
  phraseBars: 8,
  sensitivity: 5,
  brightnessPct: 80,
  leadMs: 0,
  lightsOn: true,
  helpTooltips: true,
  showInDock: "editor",
  detectorByInput: {},
  calmKickDb: 16,
  calmHighDb: 12,
  riserDb: 7,
};
const base: Status = {
  strip: { kind: "notConfigured" },
  bpm: null,
  playing: false,
  beatInBar: null,
  barInPhrase: null,
  phraseBars: 8,
  moment: "rhythm",
  auto: true,
  predicted: false,
  lightsOn: true,
  midi: { source: null, ok: true },
  audio: { ok: false },
  clip: null,
  selection: {
    set: "Default",
    setId: 1,
    paletteAuto: false,
    palette: {
      name: "Test",
      colours: [
        { h: 0, s: 1000, v: 1000 },
        { h: 240, s: 1000, v: 1000 },
        { h: 120, s: 350, v: 1000 },
      ],
    },
  },
};
const st = (p: Partial<Status>): Status => ({ ...base, ...p });

let root: HTMLElement;
let view: ReturnType<typeof mountPopover>;
const q = (s: string) => root.querySelector(s);
const text = (s: string) => q(s)?.textContent;
const byLabel = (l: string) =>
  root.querySelector<HTMLElement>(`[aria-label="${l}"]`) as HTMLElement;
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  vi.resetAllMocks();
  let stored = { ...settings };
  vi.mocked(api.getSettings).mockImplementation(() => Promise.resolve({ ...stored }));
  vi.mocked(api.saveSettings).mockImplementation((s) => Promise.resolve((stored = s)));
  vi.mocked(api.uiCommand).mockResolvedValue();
  vi.mocked(api.openEditor).mockResolvedValue();
  vi.mocked(api.quit).mockResolvedValue();
  document.body.innerHTML = '<div id="app"></div>';
  root = document.getElementById("app") as HTMLElement;
  view = mountPopover(root);
});

const sel = (over: Partial<Selection> = {}): Selection => ({ ...base.selection, ...over });

it("header_shows_bpm_beat_phrase_moment", () => {
  view.update(st({ bpm: 122.04, playing: true, beatInBar: 2, barInPhrase: 3, phraseBars: 8 }));
  expect(text(".bpm")).toBe("122.0");
  expect(text(".tempo-sub")).toBe("BPMphrase: bar 3 of 8");
  const lit = () => [...root.querySelectorAll(".beat")].map((b) => b.classList.contains("lit"));
  expect(lit()).toEqual([false, true, false, false]);
  expect(q(".beats")?.getAttribute("aria-label")).toBe("Beat 2 of 4");
  expect(text(".moment-chip")).toBe("Rhythm");
  expect(q(".moment-chip svg path")?.getAttribute("d")).toBe("M3 12h3l3-7 4 14 3-7h5");
  view.update(st({ moment: "calm" }));
  expect(q(".moment-chip svg path")?.getAttribute("d")).toBe("M2 12c3-4 5-4 8 0s5 4 8 0 3-2 4-1");
  view.update(st({ moment: "buildUp" }));
  expect(text(".moment-chip")).toBe("Build-up");
  expect(
    [...root.querySelectorAll(".moment-chip svg path")].map((x) => x.getAttribute("d")).join(""),
  ).toBe("M3 20 21 4M21 4v7M21 4h-7");
  view.update(base);
  expect(text(".bpm")).toBe("—");
  expect(text(".tempo-sub")).toBe("BPMphrase: bar — of 8");
  expect(lit()).toEqual([false, false, false, false]);
  expect(q(".beats")?.getAttribute("aria-label")).toBe("No beat");
});

it("lights_play_button_teal_on_grey_off", () => {
  view.update(
    st({
      lightsOn: true,
      strip: { kind: "connected", name: "dj lights", segments: 10, reported: 10 },
    }),
  );
  const b = byLabel("Lights");
  expect(b.getAttribute("aria-pressed")).toBe("true");
  expect(b.querySelector("svg")).not.toBeNull();
  expect(text(".lights-state")).toBe("On · following the music");
  expect(text(".strip-name")).toBe("dj lights · connected");
  b.click();
  expect(api.uiCommand).toHaveBeenLastCalledWith({ kind: "lights", on: false });
  view.update(st({ lightsOn: false }));
  expect(b.getAttribute("aria-pressed")).toBe("false");
  expect(text(".lights-state")).toBe("Off");
  expect(text(".strip-name")).toBe("not connected");
  b.click();
  expect(api.uiCommand).toHaveBeenLastCalledWith({ kind: "lights", on: true });
});

it("moment_auto_and_three_icons", () => {
  view.update(st({ moment: "calm", auto: false }));
  expect(byLabel("Auto").getAttribute("aria-pressed")).toBe("false");
  expect(byLabel("Calm").getAttribute("aria-pressed")).toBe("true");
  expect(byLabel("Rhythm").textContent).toBe("");
  expect(["Rhythm", "Calm", "Build-up"].every((l) => byLabel(l).querySelector("svg"))).toBe(true);
  byLabel("Build-up").click();
  expect(api.uiCommand).toHaveBeenCalledWith({ kind: "force", moment: "buildUp" });
  byLabel("Auto").click();
  expect(api.uiCommand).toHaveBeenCalledWith({ kind: "auto" });
  view.update(st({ auto: true }));
  expect(byLabel("Auto").getAttribute("aria-pressed")).toBe("true");
});

it("palette_shows_auto_and_cycles", async () => {
  const next = sel({
    paletteAuto: true,
    palette: { name: "Sunset", colours: base.selection.palette.colours },
  });
  vi.mocked(api.selectSet).mockResolvedValue(sel({ set: "Club" }));
  vi.mocked(api.selectPalette).mockResolvedValue(next);
  view.update(st({ selection: sel({ paletteAuto: true }) }));
  expect(text(".row-palette .row-text")).toBe("Auto");
  // Under Auto the swatches are those of the palette in force, not the set's first.
  expect(
    [...root.querySelectorAll<HTMLElement>(".swatch")].map((x) => x.style.backgroundColor),
  ).toEqual(["rgb(255, 0, 0)", "rgb(0, 0, 255)", "rgb(166, 255, 166)"]);
  expect(text(".row-set .row-text")).toBe("Default");
  byLabel("Next palette").click();
  expect(api.selectPalette).toHaveBeenCalledWith(1);
  byLabel("Previous palette").click();
  expect(api.selectPalette).toHaveBeenCalledWith(-1);
  byLabel("Next set").click();
  expect(api.selectSet).toHaveBeenCalledWith(1);
  byLabel("Previous set").click();
  expect(api.selectSet).toHaveBeenCalledWith(-1);
  await flush();
  expect(text(".row-set .row-text")).toBe("Club");
  view.update(st({ selection: sel({ paletteAuto: false }) }));
  expect(text(".row-palette .row-text")).toBe("Test");
});

it("status_dots_open_setup", () => {
  view.update(base);
  const clock = byLabel("Clock: No MIDI clock — start playback in your clock source.");
  expect(clock.querySelector(".dot")?.getAttribute("data-state")).toBe("off");
  expect(clock.title).toBe("No MIDI clock — start playback in your clock source.");
  clock.click();
  expect(api.openEditor).toHaveBeenLastCalledWith("setup");
  expect(root.querySelector('[aria-label^="Audio"] .dot')?.getAttribute("data-state")).toBe("off");
  expect(root.querySelector('[aria-label^="Strip"] .dot')?.getAttribute("data-state")).toBe("off");
  view.update(
    st({
      bpm: 120,
      playing: true,
      audio: { ok: true, device: "Loopback", levelDb: -30 },
      strip: { kind: "connected", name: "dj", segments: 10, reported: 10 },
    }),
  );
  for (const k of ["Clock", "Audio", "Strip"])
    expect(root.querySelector(`[aria-label^="${k}"] .dot`)?.getAttribute("data-state")).toBe("ok");
  view.update(st({ audio: { ok: false, failed: true }, strip: { kind: "error" } }));
  const audio = root.querySelector<HTMLElement>('[aria-label^="Audio"]') as HTMLElement;
  expect(audio.querySelector(".dot")?.getAttribute("data-state")).toBe("bad");
  expect(audio.title).toMatch(/Setup/);
  const strip = root.querySelector<HTMLElement>('[aria-label^="Strip"]') as HTMLElement;
  expect(strip.querySelector(".dot")?.getAttribute("data-state")).toBe("bad");
  strip.click();
  expect(api.openEditor).toHaveBeenLastCalledWith("strip");
  view.update(st({ strip: { kind: "connecting" } }));
  expect(strip.querySelector(".dot")?.getAttribute("data-state")).toBe("off");
});

it("strip_help_points_to_the_strip_tab_not_setup", () => {
  for (const kind of ["notConfigured", "keyInvalid", "notFound"] as const) {
    view.update(st({ strip: { kind } }));
    const title = root.querySelector<HTMLElement>('[aria-label^="Strip"]')?.title ?? "";
    expect(title, kind).toMatch(/Strip tab/);
    expect(title, kind).not.toMatch(/Setup/);
  }
});

it("clock_dot_is_red_when_the_chosen_midi_source_is_missing", () => {
  view.update(st({ playing: true, bpm: 120, midi: { source: "IAC Bus", ok: false } }));
  const dot = root.querySelector('[aria-label^="Clock"] .dot');
  expect(dot?.getAttribute("data-state")).toBe("bad");
  expect(root.querySelector<HTMLElement>('[aria-label^="Clock"]')?.title).toMatch(/Setup/);
});

it("footer_opens_editor_and_quits", () => {
  byLabel("Open editor").click();
  expect(api.openEditor).toHaveBeenLastCalledWith("sets");
  expect(byLabel("Quit").querySelector("svg")).not.toBeNull();
  byLabel("Quit").click();
  expect(api.quit).toHaveBeenCalled();
});

it("brightness_slider_saves", async () => {
  await flush();
  const slider = byLabel("Brightness") as HTMLInputElement;
  expect(slider.min).toBe("10");
  expect(slider.value).toBe("80");
  slider.value = "40";
  slider.dispatchEvent(new Event("change"));
  await flush();
  expect(api.saveSettings).toHaveBeenCalledWith({ ...settings, brightnessPct: 40 });
});

it("no_settings_view_no_changes_slider", () => {
  expect(root.querySelector('[aria-label="Settings"], .view-settings, .gear')).toBeNull();
  expect(root.querySelector('[aria-label="Detection sensitivity"]')).toBeNull();
  expect(root.querySelectorAll("input[type=range]").length).toBe(1);
});

it("errors_are_shown", async () => {
  vi.mocked(api.uiCommand).mockRejectedValue("boom");
  byLabel("Auto").click();
  await flush();
  expect(text(".error")).toBe("boom");
});

it("copy_is_english", () => {
  view.update(base);
  const words = [...root.querySelectorAll("*")]
    .filter((n) => n.children.length === 0 && n.textContent)
    .map((n) => n.textContent);
  expect(words).toMatchInlineSnapshot(`
    [
      "—",
      "BPM",
      "phrase: bar — of 8",
      "Rhythm",
      "Lights",
      "On · following the music",
      "not connected",
      "Brightness",
      "Moment",
      "Auto",
      "Set",
      "‹",
      "Default",
      "›",
      "Palette",
      "‹",
      "Test",
      "›",
      "Clock",
      "Audio",
      "Strip",
      "Open editor",
    ]
  `);
});
