import { afterAll, beforeAll, expect, it } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import { api } from "./api";
import { installMock } from "./mock";
import { App } from "./App";
import { Wizard } from "./strip/Wizard";
import type { WizardState } from "./api";
import { TABS } from "./shell/TopBar";

const INTERACTIVE =
  "button, input, select, textarea, [role=slider], [draggable=true], [tabindex='0']";
const tick = () => new Promise((r) => setTimeout(r));

beforeAll(() => {
  installMock(api);
});
afterAll(() => {
  document.body.innerHTML = "";
});

const describe_ = (e: Element) =>
  `${e.tagName.toLowerCase()}[${e.getAttribute("aria-label") ?? e.getAttribute("role") ?? ""}] "${(e.textContent ?? "").trim().slice(0, 30)}"`;

async function mountTab(id: string) {
  history.replaceState(null, "", `/?section=${id}&mock`);
  const root = document.createElement("div");
  document.body.append(root);
  await act(async () => {
    render(<App />, root);
    for (let i = 0; i < 5; i++) await tick();
  });
  return root;
}
const missing = (root: HTMLElement) =>
  [...root.querySelectorAll(INTERACTIVE)].filter((e) => !e.closest("[data-help]")).map(describe_);

for (const [id] of TABS) {
  it(`every_control_on_${id}_has_hover_help`, async () => {
    const root = await mountTab(id);
    expect(root.querySelectorAll(INTERACTIVE).length).toBeGreaterThan(8);
    expect(missing(root)).toEqual([]);
    root.remove();
  });
}

it("every_control_in_the_connection_steps_has_hover_help", async () => {
  let push: (s: WizardState) => void = () => {};
  api.onWizard = async (cb) => ((push = cb), () => {});
  const root = document.createElement("div");
  document.body.append(root);
  await act(async () => {
    render(<Wizard />, root);
    await tick();
  });
  const device = { id: "a", name: "Strip", product: "P", online: true, supported: true };
  const states: WizardState[] = [
    { kind: "idle" },
    { kind: "chooseStrip", devices: [device, { ...device, id: "b", supported: false }] },
    { kind: "searching", device, sinceUs: 1, help: false },
    { kind: "identify", device, segments: 6 },
    { kind: "saved", device, segments: 6 },
    { kind: "failed", step: "login", message: "x" },
  ];
  for (const st of states) {
    await act(async () => {
      push(st);
      await tick();
    });
    expect(root.querySelectorAll(INTERACTIVE).length).toBeGreaterThan(0);
    expect(missing(root)).toEqual([]);
  }
});
