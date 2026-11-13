import { afterEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import type { Settings } from "../../api";

vi.mock("../api", () => ({
  api: { getSettings: vi.fn(), saveSettings: vi.fn() },
}));
import { api } from "../api";
import { HelpProvider, HelpToggle, help } from "./Help";

async function mount() {
  vi.mocked(api.getSettings).mockResolvedValue({ helpTooltips: true } as Settings);
  vi.mocked(api.saveSettings).mockImplementation(async (s) => s);
  const root = document.createElement("div");
  document.body.append(root);
  await act(async () => {
    render(
      <HelpProvider>
        <HelpToggle />
        <button type="button" id="b" {...help("Does the thing.")}>
          Go
        </button>
        <button type="button" id="plain">
          No help
        </button>
      </HelpProvider>,
      root,
    );
    await new Promise((r) => setTimeout(r));
  });
  return root;
}
const fire = (el: Element, type: string, init: EventInit = { bubbles: true }) =>
  act(async () => void el.dispatchEvent(new Event(type, init)));
const tip = () => document.querySelector<HTMLElement>("[role=tooltip]");

afterEach(() => {
  document.body.innerHTML = "";
  vi.resetAllMocks();
});

it("hover_shows_the_tooltip_at_once_and_links_it", async () => {
  const root = await mount();
  const b = root.querySelector("#b") as HTMLElement;
  expect(tip()).toBeNull();
  await fire(b, "mouseover");
  expect(tip()?.textContent).toBe("Does the thing.");
  expect(b.getAttribute("aria-describedby")).toBe(tip()?.id);
  expect(b.getAttribute("title")).toBeNull();
});

it("leaving_hides_the_tooltip", async () => {
  const root = await mount();
  const b = root.querySelector("#b") as HTMLElement;
  await fire(b, "mouseover");
  await fire(b, "mouseout");
  expect(tip()).toBeNull();
  expect(b.hasAttribute("aria-describedby")).toBe(false);
});

it("keyboard_focus_shows_and_blur_hides", async () => {
  const root = await mount();
  const b = root.querySelector("#b") as HTMLElement;
  await fire(b, "focusin");
  expect(tip()?.textContent).toBe("Does the thing.");
  await fire(b, "focusout");
  expect(tip()).toBeNull();
});

it("escape_hides_the_tooltip", async () => {
  const root = await mount();
  await fire(root.querySelector("#b") as Element, "mouseover");
  await act(
    async () => void document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })),
  );
  expect(tip()).toBeNull();
});

it("controls_without_help_show_nothing", async () => {
  const root = await mount();
  await fire(root.querySelector("#plain") as Element, "mouseover");
  expect(tip()).toBeNull();
});

it("no_tooltip_while_help_is_off", async () => {
  const root = await mount();
  await act(async () => root.querySelector<HTMLElement>("[aria-label=Help]")?.click());
  await fire(root.querySelector("#b") as Element, "mouseover");
  expect(tip()).toBeNull();
});

it("the_tooltip_stays_inside_the_window", async () => {
  const root = await mount();
  const b = root.querySelector("#b") as HTMLElement;
  b.getBoundingClientRect = () => ({
    left: 1000,
    right: 1020,
    top: 10,
    bottom: 30,
    width: 20,
    height: 20,
    x: 1000,
    y: 10,
    toJSON() {},
  });
  await fire(b, "mouseover");
  const left = parseFloat(tip()?.style.left ?? "NaN");
  expect(left).toBeLessThanOrEqual(window.innerWidth);
  expect(tip()?.style.position || tip()?.className).toBeTruthy();
});
