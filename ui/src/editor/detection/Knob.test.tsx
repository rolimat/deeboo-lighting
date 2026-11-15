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
const wheel = (el: HTMLElement, deltaY: number, shiftKey = false, deltaX = 0, t = 0) =>
  act(() => {
    const e = new WheelEvent("wheel", {
      deltaY,
      deltaX,
      shiftKey,
      bubbles: true,
      cancelable: true,
    });
    Object.defineProperty(e, "timeStamp", { value: t });
    el.dispatchEvent(e);
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
  await wheel(el, -100, false, 0, 1000);
  await wheel(el, -100, true, 0, 2000);
  await wheel(el, 100, false, 0, 3000);
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

it("coarse_moves_keep_the_tenths", async () => {
  const { onChange, el } = await mount(14.3);
  await key(el, "ArrowUp");
  await key(el, "ArrowDown");
  await wheel(el, -100);
  expect(onChange.mock.calls.map((c) => c[0])).toEqual([15.3, 13.3, 15.3]);
  onChange.mockClear();
  await act(() => {
    el.dispatchEvent(new MouseEvent("pointerdown", { clientY: 200, bubbles: true }));
  });
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 197 }));
  });
  expect(onChange).not.toHaveBeenCalled();
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 194 }));
  });
  expect(onChange).toHaveBeenLastCalledWith(15.3);
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointerup"));
  });
});

it("shift_wheel_works_when_the_os_sends_it_as_horizontal_scroll", async () => {
  const { onChange, el } = await mount();
  await wheel(el, 0, true, -3, 1000);
  expect(onChange).toHaveBeenLastCalledWith(16.1);
});

it("a_trackpad_swipe_moves_one_step_per_16_px", async () => {
  const { onChange, el } = await mount();
  for (let i = 0; i < 20; i++) await wheel(el, -2, false, 0, 1000 + i * 10);
  // First event at once, then 19 x 2 px = 38 px: two more steps.
  expect(onChange).toHaveBeenCalledTimes(3);
  expect(onChange.mock.calls.map((c) => c[0])).toEqual([17, 17, 17]);
});

it("a_single_small_wheel_event_after_a_pause_is_one_step", async () => {
  const { onChange, el } = await mount();
  await wheel(el, -4, false, 0, 1000);
  await wheel(el, -4, false, 0, 1300);
  expect(onChange).toHaveBeenCalledTimes(2);
});

it("line_and_page_wheel_modes_step_once_per_event", async () => {
  const { onChange, el } = await mount();
  for (let i = 0; i < 3; i++)
    await act(() => {
      const e = new WheelEvent("wheel", {
        deltaY: -1,
        deltaMode: 1,
        bubbles: true,
        cancelable: true,
      });
      Object.defineProperty(e, "timeStamp", { value: 1000 + i });
      el.dispatchEvent(e);
    });
  expect(onChange).toHaveBeenCalledTimes(3);
});

it("dragging_past_the_limit_rebases_so_reversing_moves_at_once", async () => {
  const onChange = vi.fn();
  const Host = () => {
    const [v, setV] = useState(28);
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
        onChange={(n) => {
          onChange(n);
          setV(n);
        }}
      />
    );
  };
  root = document.createElement("div");
  document.body.append(root);
  await act(async () => render(<Host />, root));
  const el = root.querySelector<HTMLElement>("[role=slider]") as HTMLElement;
  await act(() => {
    el.dispatchEvent(new MouseEvent("pointerdown", { clientY: 200, bubbles: true }));
  });
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 80 }));
  });
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 60 }));
  });
  expect(onChange.mock.calls.map((c) => c[0])).toEqual([30]);
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointermove", { clientY: 66 }));
  });
  expect(onChange).toHaveBeenLastCalledWith(29);
  await act(() => {
    window.dispatchEvent(new MouseEvent("pointerup"));
  });
});
