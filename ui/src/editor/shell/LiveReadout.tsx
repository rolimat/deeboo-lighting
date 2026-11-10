import { useEffect, useState } from "preact/hooks";
import type { MomentName, Status } from "../../api";
import { api } from "../api";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons";

const MOMENTS: Record<MomentName, [IconName, string]> = {
  rhythm: ["rhythm", "Rhythm"],
  calm: ["calm", "Calm"],
  buildUp: ["buildup", "Build-up"],
};

export function LiveReadout() {
  const [s, setS] = useState<Status | null>(null);
  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    // Listen first, then read, so no change is missed; a push is newer than the read.
    let pushed = false;
    api
      .onStatus((st) => {
        pushed = true;
        setS(st);
      })
      .then(
        (u) => (alive ? (off = u) : u()),
        () => {},
      );
    api.getFullStatus().then(
      (st) => alive && !pushed && setS(st),
      () => {},
    );
    return () => {
      alive = false;
      off?.();
    };
  }, []);
  const pill = "block min-w-0 truncate whitespace-nowrap rounded-full bg-card px-3 py-1 text-sm";
  if (!s || s.bpm === null || !s.playing)
    return (
      <span aria-label="Live" class={`${pill} text-dim`}>
        ○ No clock
      </span>
    );
  const [icon, name] = MOMENTS[s.moment];
  const parts = [`${s.bpm.toFixed(1)} BPM`];
  if (s.beatInBar !== null) parts.push(`beat ${s.beatInBar}`);
  if (s.barInPhrase !== null) parts.push(`phrase: bar ${s.barInPhrase} of ${s.phraseBars}`);
  return (
    <span aria-label="Live" class={pill}>
      <span class="text-accent">●</span>
      {` ${parts.join(" · ")} · `}
      <span class="inline-flex items-center gap-1 align-middle font-bold">
        <Icon name={icon} />
        {name}
      </span>
    </span>
  );
}
