import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Settings, Status } from "../../api";

vi.mock("../api", () => ({
  api: {
    getSettings: vi.fn(),
    saveSettings: vi.fn(),
    getFullStatus: vi.fn(),
    onStatus: vi.fn(),
    listMidiSources: vi.fn(),
    listAudioInputs: vi.fn(),
    onMidiSources: vi.fn(),
    calibrateLatency: vi.fn(async () => {}),
    quit: vi.fn(async () => {}),
  },
}));
import { api } from "../api";
import { SetupTab } from "./SetupTab";

const settings = (over: Partial<Settings> = {}): Settings => ({
  midiSource: null,
  audioInput: null,
  phraseBars: 8,
  sensitivity: 5,
  brightnessPct: 100,
  leadMs: 100,
  lightsOn: true,
  calmKickDb: 14,
  calmHighDb: 10,
  riserDb: 6,
  helpTooltips: true,
  showInDock: "editor",
  ...over,
});
const hsv = { h: 0, s: 0, v: 0 };
const status = (over: Partial<Status> = {}): Status => ({
  strip: { kind: "notFound" },
  bpm: 122,
  playing: true,
  beatInBar: 1,
  barInPhrase: 1,
  phraseBars: 8,
  moment: "rhythm",
  auto: true,
  predicted: false,
  lightsOn: true,
  midi: { source: "Clock A", ok: true },
  audio: { ok: true, device: "Mic B", levelDb: -20 },
  selection: {
    set: "",
    setId: 1,
    paletteAuto: true,
    palette: { name: "", colours: [hsv, hsv, hsv] },
  },
  clip: null,
  ...over,
});

let root: HTMLElement;
let pushStatus: (s: Status) => void = () => {};
let pushSources: (l: string[]) => void = () => {};
const settle = () => act(async () => void (await new Promise((r) => setTimeout(r))));
const mount = async (st: Status = status(), s: Settings = settings()) => {
  vi.mocked(api.getFullStatus).mockResolvedValue(st);
  vi.mocked(api.getSettings).mockResolvedValue(s);
  await act(async () => {
    render(<SetupTab />, root);
    await new Promise((r) => setTimeout(r));
  });
};
const q = <T extends HTMLElement>(sel: string) => {
  const el = root.querySelector<T>(sel);
  if (!el) throw new Error(`missing ${sel}`);
  return el;
};
const click = (el: HTMLElement) => act(async () => void el.click());
const saved = () => vi.mocked(api.saveSettings).mock.calls.at(-1)?.[0];

beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
  // Like the backend: what was saved is what the next read returns.
  vi.mocked(api.saveSettings).mockImplementation(async (s) => {
    vi.mocked(api.getSettings).mockResolvedValue(s);
    return s;
  });
  vi.mocked(api.listMidiSources).mockResolvedValue(["Clock A"]);
  vi.mocked(api.listAudioInputs).mockResolvedValue(["Mic B"]);
  vi.mocked(api.onStatus).mockImplementation(async (cb) => {
    pushStatus = cb;
    return () => {};
  });
  vi.mocked(api.onMidiSources).mockImplementation(async (cb) => {
    pushSources = cb;
    return () => {};
  });
});
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
  vi.clearAllMocks();
});

it("midi_list_updates_on_sources_event", async () => {
  await mount();
  const names = () => [...q("select[aria-label='MIDI input']").querySelectorAll("option")];
  expect(names().map((o) => o.textContent)).toEqual([
    "Virtual port (Deeboo Lighting In)",
    "Clock A",
  ]);
  await act(async () => void pushSources(["Clock A", "Late Source"]));
  expect(names().map((o) => o.textContent)).toContain("Late Source");
  expect(api.listMidiSources).toHaveBeenCalledTimes(1); // no polling
  expect(root.textContent).toContain("● Receiving · 122.0 BPM");
  q<HTMLSelectElement>("select[aria-label='MIDI input']").value = "Late Source";
  await act(
    async () =>
      void q("select[aria-label='MIDI input']").dispatchEvent(
        new Event("change", { bubbles: true }),
      ),
  );
  expect(saved()?.midiSource).toBe("Late Source");
});

it("audio_level_and_permission_status", async () => {
  await mount();
  expect(q("[role=meter]").getAttribute("aria-valuenow")).toBe("-20");
  expect(root.textContent).toContain("Level OK");
  expect(root.textContent).toContain("Microphone access: allowed");
  await act(async () => void pushStatus(status({ audio: { ok: false } })));
  expect(root.querySelector("[role=meter]")).toBeNull();
  expect(root.textContent).toContain("System Settings");
  expect(root.textContent).toContain("Privacy & Security");
  expect(root.textContent).not.toContain("Microphone access: allowed");
  expect(root.querySelector("a, [aria-label='Open System Settings']")).toBeNull();
});

it("latency_stepper_10ms_within_range", async () => {
  await mount();
  await click(q("button[aria-label='Send earlier by 10 ms more']"));
  expect(saved()?.leadMs).toBe(110);
  await click(q("button[aria-label='Send 10 ms later']"));
  await click(q("button[aria-label='Send 10 ms later']"));
  expect(saved()?.leadMs).toBe(90);
  const input = q<HTMLInputElement>("input[aria-label='Send early by (ms)']");
  input.value = "9999";
  await act(async () => void input.dispatchEvent(new Event("change", { bubbles: true })));
  await settle();
  expect(saved()?.leadMs).toBe(500);
  expect(q<HTMLButtonElement>("button[aria-label='Send earlier by 10 ms more']").disabled).toBe(
    true,
  );
  input.value = "abc";
  await act(async () => void input.dispatchEvent(new Event("change", { bubbles: true })));
  await settle();
  expect(saved()?.leadMs).toBe(500);
  expect(input.value).toBe("500");
});

const connected = { kind: "connected", name: "Strip", segments: 4, reported: 4 } as const;

it("calibrate_toggles_and_stops_on_leave", async () => {
  await mount(status({ strip: connected }));
  expect(root.textContent).not.toContain("the strip flashes");
  await click(q("button[aria-label='Calibrate']"));
  expect(api.calibrateLatency).toHaveBeenLastCalledWith(true);
  expect(root.textContent).toContain(
    "the strip flashes on every beat while the clock plays. Use − / + until the flash lands on the kick you hear. 10 ms per click.",
  );
  vi.mocked(api.calibrateLatency).mockClear();
  await act(async () => void window.dispatchEvent(new Event("blur")));
  expect(api.calibrateLatency).not.toHaveBeenCalled(); // looking at the strip or the DJ app keeps it on
  await act(async () => render(null, root));
  expect(api.calibrateLatency).toHaveBeenCalledWith(false);
});

it("calibrate_without_a_clock_says_it_flashes_at_120", async () => {
  await mount(status({ strip: connected, playing: false, bpm: null }));
  const btn = q<HTMLButtonElement>("button[aria-label='Calibrate']");
  expect(btn.disabled).toBe(false);
  await click(btn);
  expect(api.calibrateLatency).toHaveBeenLastCalledWith(true);
  expect(root.textContent).toContain("flashes at 120 BPM until a MIDI clock plays");
  await act(async () => void pushStatus(status({ strip: connected })));
  expect(api.calibrateLatency).toHaveBeenLastCalledWith(true); // a clock arriving keeps it on
});

it("calibrate_explains_what_is_missing", async () => {
  await mount(status({ strip: { kind: "notFound" } }));
  expect(q<HTMLButtonElement>("button[aria-label='Calibrate']").disabled).toBe(true);
  expect(root.textContent).toContain("Connect the strip first.");
  await act(async () => void pushStatus(status({ strip: connected, lightsOn: false })));
  expect(q<HTMLButtonElement>("button[aria-label='Calibrate']").disabled).toBe(true);
  expect(root.textContent).toContain("Turn the lights on to calibrate.");
});

it("calibrate_stops_when_the_lights_go_off", async () => {
  await mount(status({ strip: connected }));
  await click(q("button[aria-label='Calibrate']"));
  await act(async () => void pushStatus(status({ strip: connected, lightsOn: false })));
  expect(api.calibrateLatency).toHaveBeenLastCalledWith(false);
});

it("phrase_bars_4_8_16", async () => {
  await mount();
  const radios = [...root.querySelectorAll<HTMLElement>("[role=radio]")];
  expect(radios.map((r) => r.textContent)).toEqual(["4", "8", "16"]);
  expect(radios.map((r) => r.getAttribute("aria-checked"))).toEqual(["false", "true", "false"]);
  await click(radios[2] as HTMLElement);
  expect(saved()?.phraseBars).toBe(16);
});

it("show_in_dock_offers_editor_or_always_and_saves", async () => {
  await mount();
  const sel = q<HTMLSelectElement>("select[aria-label='Show in Dock']");
  expect([...sel.options].map((o) => o.textContent)).toEqual([
    "Only while the editor is open",
    "Always",
  ]);
  expect(sel.value).toBe("editor");
  sel.value = "always";
  await act(async () => void sel.dispatchEvent(new Event("change", { bubbles: true })));
  expect(saved()?.showInDock).toBe("always");
});

it("moment_changes_slider_saves_sensitivity", async () => {
  await mount();
  const r = q<HTMLInputElement>("input[aria-label='Moment changes']");
  r.value = "4";
  await act(async () => void r.dispatchEvent(new Event("input", { bubbles: true })));
  expect(saved()?.sensitivity).toBe(9);
});

it("quit_button_calls_quit", async () => {
  await mount();
  await click(q("button[aria-label='Quit Deeboo Lighting']"));
  expect(api.quit).toHaveBeenCalledTimes(1);
});

it("copy_never_names_midi_sync", async () => {
  await mount();
  await settle();
  const all =
    (root.textContent ?? "") +
    [...root.querySelectorAll("[title]")].map((e) => e.getAttribute("title")).join();
  expect(all).not.toMatch(/midi sync/i);
  for (const t of [
    "Where the tempo comes from. Deeboo Lighting listens; it never sends.",
    "What the detector listens to. Only levels per beat are measured; nothing is recorded.",
    "The strip reacts a bit late over Wi-Fi. The app sends this much earlier so light lands on the beat.",
    'How many bars count as one phrase. The counter "phrase: bar x of N" uses it.',
    "Any MIDI clock works. The last input used is remembered; new sources appear on their own, and if the source goes away it reconnects when it comes back.",
  ])
    expect(root.textContent).toContain(t);
});

it("a_save_keeps_settings_changed_elsewhere_since_mount", async () => {
  await mount();
  // The engine picked the clock source by itself after the tab opened.
  vi.mocked(api.getSettings).mockResolvedValue(
    settings({ midiSource: "Clock A", brightnessPct: 40 }),
  );
  await click(q("button[aria-label='Send earlier by 10 ms more']"));
  expect(saved()?.leadMs).toBe(110);
  expect(saved()?.midiSource).toBe("Clock A");
  expect(saved()?.brightnessPct).toBe(40);
});
