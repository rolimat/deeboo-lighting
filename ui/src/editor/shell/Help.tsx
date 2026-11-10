import { createContext } from "preact";
import { useContext, useEffect, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { api } from "../api";
import { Icon } from "../../icons/Icon";

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
  return <Ctx.Provider value={{ on, toggle }}>{children}</Ctx.Provider>;
}

/** Props for a control: a hover explanation while help is on. */
export function useHelp(text: string): { title?: string } {
  return useContext(Ctx).on ? { title: text } : {};
}

export function HelpToggle() {
  const { on, toggle } = useContext(Ctx);
  return (
    <button
      type="button"
      aria-label="Help"
      aria-pressed={on}
      title={on ? "Hover tips are on" : "Hover tips are off"}
      onClick={toggle}
      class={`grid h-7 w-7 shrink-0 place-items-center rounded-lg border ${on ? "border-accent text-accent" : "border-border text-dim"}`}
    >
      <Icon name="help" />
    </button>
  );
}
