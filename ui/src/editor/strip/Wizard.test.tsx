import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { DeviceDto, WizardState } from "../api";

vi.mock("../api", () => ({
  api: {
    wizardStart: vi.fn(),
    wizardChoose: vi.fn(),
    wizardRetry: vi.fn(),
    wizardCancel: vi.fn(),
    stripIdentify: vi.fn(),
    wizardSave: vi.fn(),
    onWizard: vi.fn(),
    // The saved step mounts the shape editor, which loads the library.
    getLibrary: vi.fn(() => new Promise(() => {})),
  },
}));
import { api } from "../api";
import { Wizard } from "./Wizard";
import { QrCanvas } from "./QrCanvas";

const dev = (id: string, supported = true): DeviceDto => ({
  id,
  name: `Strip ${id}`,
  product: "P",
  online: true,
  supported,
});

let push: (s: WizardState) => void = () => {};
let root: HTMLElement;

const settle = () => new Promise((r) => setTimeout(r));

async function mount() {
  await act(async () => {
    render(<Wizard />, root);
    await settle();
  });
}

async function emit(s: WizardState) {
  await act(async () => {
    push(s);
    await settle();
  });
}

function button(label: string): HTMLButtonElement {
  const b = [...root.querySelectorAll("button")].find((x) => x.textContent === label);
  if (!b) throw new Error(`no button ${label}`);
  return b;
}

beforeEach(() => {
  root = document.createElement("div");
  document.body.append(root);
  // jsdom has no canvas; the QR test installs its own recording context.
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  vi.mocked(api.onWizard).mockImplementation(async (cb) => {
    push = cb;
    return () => {};
  });
  for (const f of [
    api.wizardStart,
    api.wizardChoose,
    api.wizardRetry,
    api.wizardCancel,
    api.stripIdentify,
    api.wizardSave,
  ]) {
    vi.mocked(f).mockResolvedValue();
  }
});

afterEach(async () => {
  // act() flushes pending effects and cleanups before jsdom is torn down.
  await act(async () => render(null, root));
  root.remove();
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

it("renders_user_code_step_first", async () => {
  await mount();
  expect(root.textContent).toContain(
    "Open Smart Life → Me → Settings → Account and Security → User Code, then paste it here.",
  );
  expect(root.querySelector("input")).not.toBeNull();
  expect(button("Continue")).toBeTruthy();
});

it("user_code_validation_message", async () => {
  await mount();
  const msg = "Enter the User Code from Smart Life (letters and digits only).";
  await act(async () => {
    button("Continue").click();
    await settle();
  });
  expect(root.textContent).toContain(msg);
  expect(api.wizardStart).not.toHaveBeenCalled();

  // A rejection from Rust is shown, and the typed code is never echoed.
  vi.mocked(api.wizardStart).mockRejectedValue("Wizard already running.");
  const input = root.querySelector("input") as HTMLInputElement;
  await act(async () => {
    input.value = "ab12";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await settle();
  });
  await act(async () => {
    button("Continue").click();
    await settle();
  });
  expect(api.wizardStart).toHaveBeenCalledWith("ab12");
  expect(root.textContent).toContain("Wizard already running.");
  expect(root.textContent).not.toContain("ab12");
});

it("renders_qr_step_with_countdown_and_cancel", async () => {
  await mount();
  await emit({
    kind: "waitingScan",
    qr: { size: 1, modules: [true] },
    expiresAtUs: (Date.now() + 125_000) * 1000,
  });
  expect(root.textContent).toContain("Scan this code with Smart Life and confirm.");
  expect(root.textContent).toMatch(/2:0\d/);
  expect(root.querySelector("canvas")).not.toBeNull();
  await act(async () => {
    button("Cancel").click();
    await settle();
  });
  expect(api.wizardCancel).toHaveBeenCalled();
});

it("renders_choose_step_greys_unsupported", async () => {
  await mount();
  await emit({ kind: "chooseStrip", devices: [dev("a"), dev("b", false)] });
  const items = [...root.querySelectorAll("li")];
  expect(items).toHaveLength(2);
  const a = items[0]?.querySelector("button") as HTMLButtonElement;
  const b = items[1]?.querySelector("button") as HTMLButtonElement;
  expect(a.disabled).toBe(false);
  expect(b.disabled).toBe(true);
  expect(items[1]?.textContent).toContain("not supported");
  await act(async () => {
    a.click();
    await settle();
  });
  expect(api.wizardChoose).toHaveBeenCalledWith("a");
});

it("renders_searching_step_and_help_after_30s", async () => {
  await mount();
  const base = { kind: "searching", device: dev("a"), sinceUs: 1 } as const;
  await emit({ ...base, help: false });
  expect(root.textContent).toContain("Looking for Strip a on your wifi…");
  expect(root.textContent).toContain(
    "macOS may ask to allow Deeboo Lighting to find devices on your local network. Choose Allow.",
  );
  expect(root.textContent).not.toContain("Not found yet.");
  await emit({ ...base, help: true });
  expect(root.textContent).toContain(
    "Not found yet. It may be connected only by Bluetooth or on another network. In Smart Life, remove it and add it again on a 2.4 GHz wifi. Keep this window open; it will continue when the strip appears.",
  );
});

it("renders_segments_step", async () => {
  await mount();
  await emit({ kind: "listening", device: dev("a"), sinceUs: 1 });
  expect(root.textContent).toContain(
    "Open Smart Life, go to the paint screen and tap any segment.",
  );
});

it("renders_identify_step_and_lights_a_piece", async () => {
  await mount();
  await emit({ kind: "identify", device: dev("a"), segments: 3 });
  const pieces = [...root.querySelectorAll("[data-piece]")] as HTMLButtonElement[];
  expect(pieces.map((p) => p.textContent)).toEqual(["1", "2", "3"]);
  await act(async () => {
    pieces[1]?.click();
    await settle();
  });
  expect(api.stripIdentify).toHaveBeenCalledWith(2);
  await act(async () => {
    button("Save").click();
    await settle();
  });
  expect(api.wizardSave).toHaveBeenCalledWith(3);
});

it("shows_the_error_when_save_is_rejected", async () => {
  vi.mocked(api.wizardSave).mockRejectedValue("Saved. Restart the app to use the new strip.");
  await mount();
  await emit({ kind: "identify", device: dev("a"), segments: 3 });
  expect(root.querySelector("[role=alert]")).toBeNull();
  await act(async () => {
    button("Save").click();
    await settle();
  });
  expect(root.querySelector("[role=alert]")?.textContent).toBe(
    "Saved. Restart the app to use the new strip.",
  );
});

it("renders_saved_step", async () => {
  await mount();
  await emit({ kind: "saved", device: dev("a"), segments: 10 });
  expect(root.textContent).toContain("Connected: Strip a (10 segments).");
});

it("renders_failed_step_with_retry", async () => {
  await mount();
  await emit({ kind: "failed", step: "segments", message: "No paint data received." });
  expect(root.textContent).toContain("No paint data received.");
  await act(async () => {
    button("Retry").click();
    await settle();
  });
  expect(api.wizardRetry).toHaveBeenCalled();
});

it("cancel_on_close_calls_wizard_cancel", async () => {
  await mount();
  window.dispatchEvent(new Event("beforeunload"));
  expect(api.wizardCancel).toHaveBeenCalledTimes(1);
});

it("qr_canvas_draws_modules", async () => {
  const fills: [number, number, number, number][] = [];
  const ctx = {
    fillStyle: "",
    fillRect: (x: number, y: number, w: number, h: number) => fills.push([x, y, w, h]),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    ctx as unknown as CanvasRenderingContext2D,
  );
  await act(async () => {
    render(<QrCanvas qr={{ size: 2, modules: [true, false, false, true] }} />, root);
    await settle();
  });
  // One background fill, then one rect per dark module, on the diagonal.
  expect(fills).toHaveLength(3);
  const [, a, b] = fills as [unknown, number[], number[]];
  expect(a?.[0]).toBe(a?.[1]);
  expect(b?.[0]).toBe(b?.[1]);
  expect(b?.[0]).toBeGreaterThan(a?.[0] ?? 0);
});

it("copy_is_english", async () => {
  const states: WizardState[] = [
    { kind: "idle" },
    { kind: "chooseStrip", devices: [dev("a"), dev("b", false)] },
    { kind: "searching", device: dev("a"), sinceUs: 1, help: true },
    { kind: "listening", device: dev("a"), sinceUs: 1 },
    { kind: "identify", device: dev("a"), segments: 2 },
    { kind: "saved", device: dev("a"), segments: 2 },
  ];
  await mount();
  for (const s of states) {
    await emit(s);
    // Plain ASCII plus the arrows, ellipsis and middle dot used in the approved copy: no stray non-English text.
    expect(root.textContent ?? "").toMatch(/^[\x20-\x7E→…‹\u00B7]*$/);
  }
});
