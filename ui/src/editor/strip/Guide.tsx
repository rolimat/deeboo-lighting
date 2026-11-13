import { help } from "../shell/Help";
import { H } from "../help-texts";
import { useEffect, useRef, useState } from "preact/hooks";

const KEY = "deeboo.stripGuide";

function dismissed(): boolean {
  try {
    return localStorage.getItem(KEY) === "done";
  } catch {
    return false;
  }
}

/** The three-step drawing guide: shown while never dismissed or while help is on; "Got it" hides it for now. */
export function Guide({ helpOn }: { helpOn: boolean }) {
  const [closed, setClosed] = useState(false);
  const was = useRef(helpOn);
  useEffect(() => {
    if (helpOn && !was.current) setClosed(false);
    was.current = helpOn;
  }, [helpOn]);
  if (closed || !(helpOn || !dismissed())) return null;
  const close = () => {
    try {
      localStorage.setItem(KEY, "done");
    } catch {
      // Private mode: the guide simply comes back next time.
    }
    setClosed(true);
  };
  return (
    <aside
      aria-label="How to draw your strip"
      class="absolute top-2 right-2 z-10 w-60 rounded-card border border-accent bg-card p-3 text-small"
    >
      <b class="t-head">How to draw your strip</b>
      <ol class="my-2 ml-4 list-decimal space-y-1">
        <li>Pick the preset closest to how the strip is mounted.</li>
        <li>
          Drag pieces in any direction; they snap to the grid and to each other. Drag the round
          handle to rotate.
        </li>
        <li>
          Click a piece: that segment lights up on the real strip, so you can check it matches.
        </li>
      </ol>
      <button type="button" onClick={close} class="text-dim underline" {...help(H.guideClose)}>
        Got it
      </button>
    </aside>
  );
}
