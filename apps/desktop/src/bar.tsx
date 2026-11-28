import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DictationPill, type PillState } from "@vocalis/ui";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./app.css";
import { play } from "./sounds";

type BarState = "hidden" | "recording" | "processing" | "slow_connection" | "near_limit" | "error";

const PILL: Record<Exclude<BarState, "hidden">, PillState> = {
  recording: "recording",
  processing: "processing",
  slow_connection: "slow",
  near_limit: "limit",
  error: "error",
};

/** The dictation bar: driven by engine events; a static preview in a browser. */
function Bar() {
  const [state, setState] = useState<BarState>("recording");
  const [levels, setLevels] = useState(() => new Array<number>(24).fill(0.05));

  useEffect(() => {
    if (!isTauri()) return;
    const subs = [
      listen<BarState>("bar-state", (e) => {
        setState(e.payload);
        if (e.payload === "recording") setLevels(new Array<number>(24).fill(0.05));
      }),
      listen<number>("bar-level", (e) => setLevels((l) => [...l.slice(1), e.payload])),
      listen<string>("sound", (e) => play(e.payload)),
    ];
    return () => subs.forEach((p) => void p.then((off) => off()));
  }, []);

  if (state === "hidden") return null;
  return <DictationPill state={PILL[state]} levels={levels} />;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Bar />
  </StrictMode>,
);
