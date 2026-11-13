import { createContext } from "preact";
import { useContext, useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { api } from "../api";
import { Icon } from "../../icons/Icon";
import { H } from "../help-texts";

interface HelpCtx {
  on: boolean;
  toggle: () => void;
}
const Ctx = createContext<HelpCtx>({ on: false, toggle: () => {} });

/** Holds the "?" toggle; it lives in `Settings.helpTooltips` so it is remembered between sessions. */
export function HelpProvider({ children }: { children: ComponentChildren }) {
  const [on, setOn] = useState(true);
  useEffect(() => {
    api.getSettings().then(
      (s) => setOn(s.helpTooltips),
      () => {},
    );
  }, []);
  const toggle = () => {
    const next = !on;
    setOn(next);
    void api
      .getSettings()
      .then((s) => api.saveSettings({ ...s, helpTooltips: next }))
      .catch(() => setOn(!next));
  };
  return (
    <Ctx.Provider value={{ on, toggle }}>
      {children}
      {on && <Tooltip />}
    </Ctx.Provider>
  );
}

/** Marks a control with its explanation; the one tooltip below reads `data-help` (works inside loops). */
export function help(text: string): { "data-help": string } {
  return { "data-help": text };
}

const TIP_ID = "help-tooltip";
const GAP = 6;
const EDGE = 4;

/** The one app-styled bubble: shown at once on hover or keyboard focus of a `[data-help]` element while help is on. */
function Tooltip() {
  const [el, setEl] = useState<HTMLElement | null>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const at = (t: EventTarget | null) =>
      setEl(t instanceof Element ? t.closest<HTMLElement>("[data-help]") : null);
    const over = (e: Event) => at(e.target);
    const hide = () => setEl(null);
    const key = (e: KeyboardEvent) => e.key === "Escape" && hide();
    document.addEventListener("mouseover", over);
    document.addEventListener("focusin", over);
    document.addEventListener("mouseout", hide);
    document.addEventListener("focusout", hide);
    document.addEventListener("keydown", key);
    if (import.meta.env.DEV) {
      // Visual review only: `?tip=<css selector>` opens that control's tooltip.
      const sel = new URLSearchParams(location.search).get("tip");
      if (sel) setTimeout(() => at(document.querySelector(sel)), 300);
    }
    return () => {
      document.removeEventListener("mouseover", over);
      document.removeEventListener("focusin", over);
      document.removeEventListener("mouseout", hide);
      document.removeEventListener("focusout", hide);
      document.removeEventListener("keydown", key);
    };
  }, []);
  useEffect(() => {
    el?.setAttribute("aria-describedby", TIP_ID);
    return () => el?.removeAttribute("aria-describedby");
  }, [el]);
  useLayoutEffect(() => {
    const tip = ref.current;
    if (!el || !tip) return;
    const r = el.getBoundingClientRect();
    const w = tip.offsetWidth;
    const h = tip.offsetHeight;
    const left = Math.max(
      EDGE,
      Math.min(r.left + r.width / 2 - w / 2, window.innerWidth - w - EDGE),
    );
    const below = r.bottom + GAP + h <= window.innerHeight;
    setPos({ left, top: below ? r.bottom + GAP : Math.max(EDGE, r.top - GAP - h) });
  }, [el]);
  if (!el) return null;
  return (
    <div
      ref={ref}
      id={TIP_ID}
      role="tooltip"
      style={{ left: `${pos.left}px`, top: `${pos.top}px` }}
      class="pointer-events-none fixed z-50 max-w-72 rounded-lg border border-border bg-card px-3 py-2 text-small text-text"
    >
      {el.dataset.help}
    </div>
  );
}

/** Whether hover help is switched on. */
export function useHelpOn(): boolean {
  return useContext(Ctx).on;
}

export function HelpToggle() {
  const { on, toggle } = useContext(Ctx);
  return (
    <button
      type="button"
      aria-label="Help"
      aria-pressed={on}
      {...help(H.helpToggle)}
      onClick={toggle}
      class={`grid h-7 w-7 shrink-0 place-items-center rounded-lg border ${on ? "border-accent text-accent" : "border-border text-dim"}`}
    >
      <Icon name="help" />
    </button>
  );
}
