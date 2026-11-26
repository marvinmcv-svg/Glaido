import { DictationPill } from "@vocalis/ui";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./app.css";

/** P0 preview of the dictation bar: animated levels, no audio capture yet. */
function Bar() {
  const [levels, setLevels] = useState<number[]>(() => Array(24).fill(0.1));
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let t = 0;
    const id = setInterval(() => {
      t += 1;
      setLevels((l) => [...l.slice(1), 0.25 + 0.6 * Math.abs(Math.sin(t / 3)) * Math.random()]);
    }, 60);
    return () => clearInterval(id);
  }, []);
  return <DictationPill state="recording" levels={levels} />;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Bar />
  </StrictMode>,
);
