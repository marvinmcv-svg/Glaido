import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@vocalis/ui";
import { useEffect, useState } from "react";
import { type Notice, type Toast, toastFor } from "./history";

type Shown = Toast & { key: number };
const LIFETIME_MS = 8000;

/** In-app notices from the engine (Appendix C), with Retry and Copy actions. */
export function Toasts() {
  const [toasts, setToasts] = useState<Shown[]>([]);
  const dismiss = (key: number) => setToasts((t) => t.filter((x) => x.key !== key));

  useEffect(() => {
    if (!isTauri()) return;
    let n = 0;
    const off = listen<Notice>("notice", (e) => {
      const key = ++n;
      setToasts((t) => [...t.slice(-3), { ...toastFor(e.payload), key }]);
      setTimeout(() => dismiss(key), LIFETIME_MS);
    });
    return () => void off.then((f) => f());
  }, []);

  const run = async (t: Shown) => {
    if (!t.recordId || !t.action) return;
    try {
      await invoke(t.action === "retry" ? "history_retry" : "history_copy", { id: t.recordId });
      dismiss(t.key);
    } catch {
      /* the record may be gone; leave the toast so the user sees nothing happened */
    }
  };

  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.key} className="toast">
          <div>
            <strong>{t.title}</strong>
            <div className="muted">{t.body}</div>
          </div>
          <div className="toast-actions">
            {t.action ? (
              <Button variant="ghost" onClick={() => void run(t)}>
                {t.action === "retry" ? "Retry" : "Copy"}
              </Button>
            ) : null}
            <Button variant="ghost" aria-label="Dismiss" onClick={() => dismiss(t.key)}>
              ✕
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}
