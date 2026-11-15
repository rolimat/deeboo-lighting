import { afterEach, expect, it, vi } from "vitest";
import { act } from "preact/test-utils";
import { render } from "preact";
import { useState } from "preact/hooks";
import { Knob } from "./Knob";

let root: HTMLElement;
afterEach(async () => {
  await act(async () => render(null, root));
  root.remove();
});
const mount = async (value = 16, max = 30) => {
  const onChange = vi.fn();
  root = document.createElement("div");
  document.body.append(root);
  await act(async () =>
    render(
      <Knob
        name="Kick drop"
        label="Calm"
        above={false}
        value={value}
        min={0}
        max={max}
        def={14}
        hint="Hint."
        onChange={onChange}
      />,
      root,
    ),
  );
  return { onChange, el: root.querySelector<HTMLElement>("[role=slider]") as HTMLElement };
};
const key = (el: HTMLElement, k: string, shiftKey = false) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent("keydown", { key: k, shiftKey, bubbles: true }));
  });
const wheel = (el: HTMLElement, deltaY: number, shiftKey = false) =>
  act(() => {
    el.dispatchEvent(
      new WheelEvent("wheel", { deltaY, shiftKey, bubbles: true, cancelable: true }),
    );
  });

it("arrows_step_by_one_and_shift_by_a_tenth", async () => {
  const { onChange, el } = await mount();
  await key(el, "ArrowUp");
  await key(el, "ArrowLeft");
  await key(el, "ArrowUp", true);
  await key(el, "ArrowDown", true);
  expect(onChange.mock.calls.map((c) => c[0])).toEqual([17, 15, 16.1, 15.9]);
});

it("three_shift_steps_from_16_make_exactly_16_3", async () => {
  let latest = 0;
  const Host = () => {
    const [v, setV] = useState(16);
    latest = v;
    return (
      <Knob
        name="K"
        label="Calm"
        above={false}
        value={v}
        min={0}
        max={30}
        def={14}
        hint="H"
        onChange={setV}
      />
    );
  };
  root = document.createElement("div");
  document.body.append(root);
  await act(async () => render(<Host />, root));
  const el = root.querySelector<HTMLElement>("[role=slider]") as HTMLElement;
  for (let i = 0; i < 3; i++) await key(el, "ArrowUp", true);
  expect(latest).toBe(16.3);
});

it("clamps_at_max", async () => {
  const { onChange, el } = await mount(30);
  await key(el, "ArrowUp");
  expect(onChange).toHaveBeenCalledWith(30);
});

it("wheel_steps_and_prevents_scroll", async () => {
  const { onChange, el } = await mount();
  await wheel(el, -100);
  await wheel(el, -100, true);
  await wheel(el, 100);
  expect(onChange.mock.calls.map((c) => c[0])).toEqual([17, 16.1, 15]);
});

it("double_click_resets", async () => {
  const { onChange, el } = await mount();
  await act(() => {
    el.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  });
  expect(onChange).toHaveBeenCalledWith(14);
});

it("drag_up_60px_adds_ten_and_clamps", async () => {
  const { onChange, el } = await mount(16, 30);
  await act(() => {
    el.dispatchEvent(new MouseEvent("pointerdown", { clientY: 200, bubbles: true }));
  });
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 140 }));
  });
  expect(onChange).toHaveBeenLastCalledWith(26);
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 0 }));
  });
  expect(onChange).toHaveBeenLastCalledWith(30);
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointerup"));
  });
  onChange.mockClear();
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 100 }));
  });
  expect(onChange).not.toHaveBeenCalled();
});

it("shift_mid_drag_does_not_jump", async () => {
  let latest = 0;
  const Host = () => {
    const [v, setV] = useState(16);
    latest = v;
    return (
      <Knob
        name="K"
        label="Calm"
        above={false}
        value={v}
        min={0}
        max={30}
        def={14}
        hint="H"
        onChange={setV}
      />
    );
  };
  root = document.createElement("div");
  document.body.append(root);
  await act(async () => render(<Host />, root));
  const el = root.querySelector<HTMLElement>("[role=slider]") as HTMLElement;
  const move = (clientY: number, shiftKey = false) =>
    act(() => {
      window.dispatchEvent(new MouseEvent("pointermove", { clientY, shiftKey }));
    });
  await act(() => {
    el.dispatchEvent(new MouseEvent("pointerdown", { clientY: 200, bubbles: true }));
  });
  await move(140);
  expect(latest).toBe(26);
  await move(134, true);
  expect(latest).toBe(26);
  await move(128, true);
  expect(latest).toBe(26.1);
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointerup"));
  });
});

it("exposes_aria_and_help", async () => {
  const { el } = await mount(14.3);
  expect(el.getAttribute("aria-valuetext")).toBe("Calm when 14.3 dB below");
  expect(el.getAttribute("aria-valuenow")).toBe("14.3");
  expect(el.getAttribute("aria-label")).toBe("Kick drop");
  expect(el.getAttribute("data-help")).toContain("Shift");
  expect(el.textContent).toContain("14.3 dB");
});
