import { afterEach, expect, it } from "vitest";
import { DRAG_TYPE, installDropGuard } from "./drag";

let off: (() => void) | undefined;
afterEach(() => off?.());

const fire = (target: EventTarget, type: string, dataTransfer: object = {}) => {
  const e = Object.assign(new Event(type, { bubbles: true, cancelable: true }), { dataTransfer });
  target.dispatchEvent(e);
  return e;
};

it("a_drop_outside_any_drop_zone_is_cancelled_so_nothing_loads_or_is_typed", () => {
  off = installDropGuard();
  const input = document.body.appendChild(document.createElement("input"));
  const dt = { dropEffect: "copy" };
  expect(fire(input, "dragover", dt).defaultPrevented).toBe(true);
  expect(dt.dropEffect).toBe("none");
  expect(fire(input, "drop").defaultPrevented).toBe(true);
  expect(fire(document.body, "drop").defaultPrevented).toBe(true);
  input.remove();
});

it("an_internal_drop_zone_keeps_its_own_dragover_effect", () => {
  off = installDropGuard();
  const zone = document.body.appendChild(document.createElement("div"));
  zone.addEventListener("dragover", (e) => e.preventDefault());
  const dt = { dropEffect: "move" };
  fire(zone, "dragover", dt);
  expect(dt.dropEffect).toBe("move");
  zone.remove();
});

it("the_guard_is_removed_on_cleanup", () => {
  installDropGuard()();
  expect(fire(document.body, "drop").defaultPrevented).toBe(false);
});

it("internal_drags_use_a_custom_type_inputs_ignore", () => {
  expect(DRAG_TYPE).not.toBe("text/plain");
});
