import { invoke, isTauri } from "@tauri-apps/api/core";
import { Button } from "@vocalis/ui";
import { useEffect, useRef, useState } from "react";

const CONFIRM_WORD = "delete";

/** Settings → Clear history, with a typed confirmation because it can't be undone (spec 4.6). */
export function ClearHistoryRow() {
  const [asking, setAsking] = useState(false);
  const [done, setDone] = useState<string | null>(null);
  return (
    <div className="row" id="clear-history">
      <div>
        <div>Clear history</div>
        <div className="muted">
          {done ?? "Permanently delete every transcript and recording on this device."}
        </div>
      </div>
      <Button variant="ghost" onClick={() => setAsking(true)}>
        Clear history…
      </Button>
      {asking ? (
        <ConfirmClear
          onCancel={() => setAsking(false)}
          onCleared={(n) => {
            setAsking(false);
            setDone(
              n === 1
                ? "History cleared, including 1 recording."
                : `History cleared, including ${n} recordings.`,
            );
          }}
        />
      ) : null}
    </div>
  );
}

function ConfirmClear({
  onCancel,
  onCleared,
}: {
  onCancel: () => void;
  onCleared: (recordings: number) => void;
}) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  const ready = typed.trim().toLowerCase() === CONFIRM_WORD;

  const clear = async () => {
    if (!ready || !isTauri()) return;
    setBusy(true);
    try {
      onCleared(await invoke<number>("history_clear_all"));
    } catch (e) {
      setError(typeof e === "string" ? e : "Couldn't clear history.");
      setBusy(false);
    }
  };

  return (
    <div className="wizard">
      <div
        className="wizard-panel dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="clear-title"
        aria-describedby="clear-body"
      >
        <h1 id="clear-title">Delete all history?</h1>
        <p id="clear-body">
          Every transcript, command and recording stored on this device is deleted for good,
          including failed recordings you could still retry. This can&apos;t be undone. Your
          dictionary, snippets and settings stay.
        </p>
        <label className="field">
          Type <strong>{CONFIRM_WORD}</strong> to confirm
          <input
            ref={input}
            className="input"
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") onCancel();
              if (e.key === "Enter") void clear();
            }}
          />
        </label>
        {error ? (
          <p role="alert" className="error-text">
            {error}
          </p>
        ) : null}
        <div className="wizard-nav">
          <span className="spacer" />
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="attention" disabled={!ready || busy} onClick={() => void clear()}>
            {busy ? "Deleting…" : "Delete everything"}
          </Button>
        </div>
      </div>
    </div>
  );
}
