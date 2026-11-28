"use client";

import { Button } from "@vocalis/ui";
import { useState } from "react";
import { approveDesktop } from "./actions";

/** Confirm, then hand the one-time code to the desktop app via its deep link. */
export function OpenApp({ challenge, state }: { challenge: string; state: string }) {
  const [busy, setBusy] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const open = async () => {
    setBusy(true);
    setError(null);
    const r = await approveDesktop(challenge, state);
    setBusy(false);
    if ("error" in r) return setError(r.error);
    setUrl(r.url);
    window.location.href = r.url;
  };

  if (url)
    return (
      <div className="notice stack" role="status">
        <div>You're signed in to the app. You can close this tab.</div>
        <div>
          Nothing happened? <a href={url}>Open Vocalis</a>. The link works once, for five minutes.
        </div>
      </div>
    );
  return (
    <div className="stack">
      {error ? (
        <div className="notice" data-kind="error" role="alert">
          {error}
        </div>
      ) : null}
      <Button onClick={() => void open()} disabled={busy}>
        {busy ? "Opening…" : "Open Vocalis"}
      </Button>
    </div>
  );
}
