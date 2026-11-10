import { afterEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import { ChangesSlider, sensitivityOf, stepOf } from "./ChangesSlider";

let root: HTMLElement;
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
});
const mount = async (sensitivity: number, onChange = vi.fn()) => {
  root = document.createElement("div");
  document.body.append(root);
  await act(async () =>
    render(<ChangesSlider sensitivity={sensitivity} onChange={onChange} />, root),
  );
  return onChange;
};

it("changes_slider_maps_to_sensitivity", async () => {
  expect([0, 1, 2, 3, 4].map(sensitivityOf)).toEqual([1, 3, 5, 7, 9]);
  expect([1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(stepOf)).toEqual([0, 0, 1, 1, 2, 2, 3, 3, 4, 4]);
  const onChange = await mount(5);
  const input = root.querySelector<HTMLInputElement>("input[type=range]");
  expect(input?.value).toBe("2");
  expect(input?.max).toBe("4");
  await act(() => {
    if (!input) return;
    input.value = "4";
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  expect(onChange).toHaveBeenCalledWith(9);
});

it("names_the_step_and_explains_it", async () => {
  await mount(5);
  expect(root.textContent).toContain("Steady");
  expect(root.textContent).toContain("Reactive");
  expect(root.querySelector("[data-current]")?.textContent).toBe("Balanced");
  expect(root.textContent).toContain(
    "Moves to Calm or Build-up only when the music clearly changes.",
  );
});

it("in_between_steps_have_their_own_explanation", async () => {
  await mount(3);
  expect(root.textContent).toContain("Changes a bit later than Balanced.");
  expect(root.querySelector("[data-current]")).toBeNull();
});
