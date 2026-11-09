import { beforeEach, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  api: {
    uiCommand: vi.fn(),
    getSettings: vi.fn(),
    saveSettings: vi.fn(),
    listMidiSources: vi.fn(),
    listAudioInputs: vi.fn(),
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
  selection: {
    set: "Default",
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
  vi.mocked(api.listMidiSources).mockResolvedValue(["IAC Bus 1", "Pioneer DJ"]);
  vi.mocked(api.listAudioInputs).mockResolvedValue(["Built-in Mic", "Loopback"]);
  vi.mocked(api.uiCommand).mockResolvedValue();
  vi.mocked(api.openEditor).mockResolvedValue();
  document.body.innerHTML = '<div id="app"></div>';
  root = document.getElementById("app") as HTMLElement;
  view = mountPopover(root);
});

it("shows_bpm_beats_and_bar", () => {
  view.update(st({ bpm: 126.04, playing: true, beatInBar: 3, barInPhrase: 3, phraseBars: 8 }));
  expect(text(".bpm")).toBe("126.0");
  const beats = [...root.querySelectorAll(".beat")];
  expect(beats.map((b) => b.classList.contains("lit"))).toEqual([false, false, true, false]);
  view.update(st({ bpm: 126, beatInBar: 1, barInPhrase: 1 }));
  expect(root.querySelector(".beat")?.classList.contains("accent")).toBe(true);
  expect(text(".bar")).toBe("bar 1 / 8");
});

it("dash_without_bpm", () => {
  view.update(base);
  expect(text(".bpm")).toBe("—");
  expect(root.querySelector(".beat.lit")).toBeNull();
  expect(text(".bar")).toBe("bar — / 8");
});

it("lights_buttons_send_commands", () => {
  view.update(st({ lightsOn: true }));
  expect(byLabel("Lights on").getAttribute("aria-pressed")).toBe("true");
  expect(byLabel("Lights off").getAttribute("aria-pressed")).toBe("false");
  byLabel("Lights off").click();
  expect(api.uiCommand).toHaveBeenCalledWith({ kind: "lights", on: false });
  byLabel("Lights on").click();
  expect(api.uiCommand).toHaveBeenCalledWith({ kind: "lights", on: true });
});

it("moment_click_forces_and_auto_toggles", () => {
  view.update(st({ moment: "calm", auto: false, predicted: true }));
  expect(byLabel("Calm").getAttribute("aria-pressed")).toBe("true");
  expect(byLabel("Rhythm").getAttribute("aria-pressed")).toBe("false");
  expect(byLabel("Auto").getAttribute("aria-pressed")).toBe("false");
  expect(text(".predicted")).toBe("drop predicted");
  byLabel("Build-up").click();
  expect(api.uiCommand).toHaveBeenCalledWith({ kind: "force", moment: "buildUp" });
  byLabel("Auto").click();
  expect(api.uiCommand).toHaveBeenCalledWith({ kind: "auto" });
  view.update(st({ auto: true }));
  expect(byLabel("Auto").textContent).toBe("Auto ✓");
  expect(q(".predicted")?.hasAttribute("hidden")).toBe(true);
});

it("status_rows_render_each_state", () => {
  view.update(base);
  expect(text(".row-midi")).toBe("MIDI · Deeboo Lighting In");
  expect(text(".row-strip .row-text")).toBe("Strip · not connected");
  expect(text(".row-audio")).toBe("Audio · off");
  view.update(
    st({
      midi: { source: "Pioneer DJ", ok: true },
      strip: { kind: "connected", name: "dj lights", segments: 10 },
      audio: { ok: true, device: "Loopback", levelDb: -30 },
    }),
  );
  expect(text(".row-midi")).toBe("MIDI · Pioneer DJ");
  expect(text(".row-strip .row-text")).toBe("Strip · dj lights (10 segments)");
  expect(text(".row-audio")).toBe("Audio · Loopback");
  expect((q(".level-fill") as HTMLElement).style.width).toBe("50%");
  view.update(st({ strip: { kind: "error" }, audio: { ok: false, device: "Loopback" } }));
  expect(text(".row-strip .row-text")).toBe("Strip · connection error");
  expect(text(".row-audio")).toBe("Audio · no access");
  view.update(st({ strip: { kind: "connecting" } }));
  expect(text(".row-strip .row-text")).toBe("Strip · connecting…");
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

it("settings_lists_sources_and_saves_changes", async () => {
  byLabel("Settings").click();
  await flush();
  const midi = byLabel("MIDI input") as HTMLSelectElement;
  expect([...midi.options].map((o) => o.textContent)).toEqual([
    "Deeboo Lighting In only",
    "IAC Bus 1",
    "Pioneer DJ",
  ]);
  const audio = byLabel("Audio input") as HTMLSelectElement;
  expect([...audio.options].map((o) => o.textContent)).toEqual([
    "System default",
    "Built-in Mic",
    "Loopback",
  ]);
  midi.value = "Pioneer DJ";
  midi.dispatchEvent(new Event("change"));
  await flush();
  expect(api.saveSettings).toHaveBeenLastCalledWith({ ...settings, midiSource: "Pioneer DJ" });
  audio.value = "";
  audio.dispatchEvent(new Event("change"));
  byLabel("16 bars").click();
  await flush();
  expect(api.saveSettings).toHaveBeenLastCalledWith({
    ...settings,
    midiSource: "Pioneer DJ",
    phraseBars: 16,
  });
  const sens = byLabel("Detection sensitivity") as HTMLInputElement;
  sens.value = "7";
  sens.dispatchEvent(new Event("change"));
  const lat = byLabel("Light latency") as HTMLInputElement;
  lat.value = "120";
  lat.dispatchEvent(new Event("change"));
  await flush();
  expect(api.saveSettings).toHaveBeenLastCalledWith({
    ...settings,
    midiSource: "Pioneer DJ",
    phraseBars: 16,
    sensitivity: 7,
    leadMs: 120,
  });
  byLabel("Quit").click();
  expect(api.quit).toHaveBeenCalled();
  byLabel("Back").click();
  expect(q(".view-settings")?.hasAttribute("hidden")).toBe(true);
});

it("settings_shows_strip_and_change_button", async () => {
  view.update(st({ strip: { kind: "connected", name: "Strip A", segments: 10 } }));
  byLabel("Settings").click();
  await flush();
  expect(text(".view-settings .strip-line")).toBe("Strip · Strip A (10)");
  const change = byLabel("Change strip");
  expect(change.textContent).toBe("Change…");
  change.click();
  expect(api.openEditor).toHaveBeenCalledWith("strip");
});

it("settings_offers_connect_without_a_strip", async () => {
  view.update(base);
  byLabel("Settings").click();
  await flush();
  expect(text(".view-settings .strip-line")).toBe("Strip · not connected");
  expect(byLabel("Change strip").textContent).toBe("Connect…");
});

it("status_offers_reconnect_on_key_invalid", () => {
  const reconnect = byLabel("Reconnect strip");
  expect(reconnect.hidden).toBe(true);
  view.update(st({ strip: { kind: "keyInvalid" } }));
  expect(text(".row-strip .row-text")).toBe("Strip · key no longer valid");
  expect(q(".row-strip .dot")?.getAttribute("data-state")).toBe("bad");
  expect(reconnect.hidden).toBe(false);
  expect(reconnect.textContent).toBe("Reconnect…");
  reconnect.click();
  expect(api.openEditor).toHaveBeenCalledWith("strip");
  view.update(st({ strip: { kind: "notFound" } }));
  expect(text(".row-strip .row-text")).toBe("Strip · not found");
  expect(reconnect.hidden).toBe(false);
  view.update(st({ strip: { kind: "connected", name: "dj", segments: 10 } }));
  expect(reconnect.hidden).toBe(true);
});

it("settings_save_error_is_shown", async () => {
  vi.mocked(api.saveSettings).mockRejectedValue("disk full");
  byLabel("Settings").click();
  await flush();
  byLabel("4 bars").click();
  await flush();
  expect(text(".view-settings .error")).toBe("disk full");
});

it("copy_is_english", async () => {
  byLabel("Settings").click();
  await flush();
  view.update(base);
  const words = [...root.querySelectorAll("*")]
    .filter((n) => n.children.length === 0 && n.textContent)
    .map((n) => n.textContent);
  expect(words).toMatchInlineSnapshot(`
    [
      "Deeboo Lighting",
      "⚙",
      "—",
      "bar — / 8",
      "Lights ▶",
      "Off",
      "Rhythm",
      "Calm",
      "Build-up",
      "Auto ✓",
      "drop predicted",
      "Set",
      "◀",
      "Default",
      "▶",
      "Palette",
      "◀",
      "Test",
      "▶",
      "Open editor…",
      "Brightness",
      "MIDI · Deeboo Lighting In",
      "Strip · not connected",
      "Reconnect…",
      "Audio · off",
      "‹ Settings",
      "Strip · not connected",
      "Connect…",
      "MIDI input",
      "Deeboo Lighting In only",
      "IAC Bus 1",
      "Pioneer DJ",
      "Audio input",
      "System default",
      "Built-in Mic",
      "Loopback",
      "Phrase length",
      "4",
      "8",
      "16",
      "Detection sensitivity",
      "Light latency (ms)",
      "Quit",
    ]
  `);
});

it("set_and_palette_rows_cycle", async () => {
  const next: Selection = {
    set: "Club",
    paletteAuto: false,
    palette: {
      name: "Sunset",
      colours: [
        { h: 30, s: 1000, v: 1000 },
        { h: 200, s: 500, v: 800 },
        { h: 0, s: 0, v: 1000 },
      ],
    },
  };
  vi.mocked(api.selectSet).mockResolvedValue(next);
  vi.mocked(api.selectPalette).mockResolvedValue(next);
  view.update(base);
  expect(text(".row-set .row-text")).toBe("Default");
  expect(text(".row-palette .row-text")).toBe("Test");
  const sw = [...root.querySelectorAll<HTMLElement>(".swatch")];
  expect(sw.map((x) => x.style.backgroundColor)).toEqual([
    "rgb(255, 0, 0)",
    "rgb(0, 0, 255)",
    "rgb(166, 255, 166)",
  ]);
  byLabel("Next set").click();
  expect(api.selectSet).toHaveBeenCalledWith(1);
  byLabel("Previous set").click();
  expect(api.selectSet).toHaveBeenCalledWith(-1);
  byLabel("Next palette").click();
  expect(api.selectPalette).toHaveBeenCalledWith(1);
  byLabel("Previous palette").click();
  expect(api.selectPalette).toHaveBeenCalledWith(-1);
  // The command's answer repaints at once; the next status push says the same.
  await flush();
  expect(text(".row-set .row-text")).toBe("Club");
  expect(text(".row-palette .row-text")).toBe("Sunset");
});

it("palette_row_reads_auto_when_nothing_is_pinned", () => {
  const sel = base.selection;
  view.update({ ...base, selection: { ...sel, paletteAuto: true } });
  expect(text(".row-palette .row-text")).toBe("Auto");
});

it("open_editor_button", () => {
  const b = byLabel("Open editor");
  expect(b.textContent).toBe("Open editor…");
  b.click();
  expect(api.openEditor).toHaveBeenCalledWith("strip");
});
