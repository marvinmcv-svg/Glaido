import { invoke, isTauri } from "@tauri-apps/api/core";
import { Button, Card, Keycap, StatCard, Switch } from "@vocalis/ui";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import icon from "../assets/app-icon.svg";
import "./app.css";
import { currentPlatform, DEFAULT_HOTKEYS } from "./platform";

type AppInfo = { name: string; version: string; coreVersion: string };
const PAGES = ["Home", "Dictionary", "Snippets", "Formatting", "Settings"] as const;

function App() {
  const [page, setPage] = useState<(typeof PAGES)[number]>("Home");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [sounds, setSounds] = useState(true);
  const platform = currentPlatform();
  const keys = DEFAULT_HOTKEYS[platform];

  useEffect(() => {
    if (isTauri())
      invoke<AppInfo>("app_info")
        .then(setInfo)
        .catch(() => setInfo(null));
  }, []);

  return (
    <div className="shell">
      <nav className="nav" aria-label="Main">
        <div className="brand">
          <img src={icon} alt="" /> Vocalis
        </div>
        {PAGES.map((p) => (
          <button key={p} aria-current={p === page ? "page" : undefined} onClick={() => setPage(p)}>
            {p}
          </button>
        ))}
      </nav>
      <main className="content">
        <h1>{page}</h1>
        {page === "Home" ? (
          <>
            <div className="stats">
              <StatCard
                label="Time saved"
                value="0m"
                hint="Your first dictation starts the clock"
              />
              <StatCard label="Speed" value="— wpm" hint="Measured after a few dictations" />
              <StatCard label="Streak" value="0 days" hint="Dictate today to start one" />
            </div>
            <Card>
              <div className="row">
                <div>
                  <div>Dictate</div>
                  <div className="muted">
                    Hold, speak, release. The text lands where your cursor is.
                  </div>
                </div>
                <Keycap keys={keys.dictation} platform={platform} />
              </div>
              <div className="row">
                <div>
                  <div>Hands-free</div>
                  <div className="muted">Press once to start, again to stop. Esc cancels.</div>
                </div>
                <Keycap keys={keys.handsFree} platform={platform} />
              </div>
              <div className="row">
                <div>
                  <div>Preview the dictation bar</div>
                  <div className="muted">Shows the bar without recording anything.</div>
                </div>
                <Button variant="ghost" onClick={() => isTauri() && invoke("toggle_bar")}>
                  Toggle bar
                </Button>
              </div>
            </Card>
          </>
        ) : page === "Settings" ? (
          <Card>
            <div className="row">
              <div>Interaction sounds</div>
              <Switch checked={sounds} onChange={setSounds} label="Interaction sounds" />
            </div>
            <div className="row">
              <div className="muted">Version</div>
              <div className="muted">
                {info ? `${info.version} (core ${info.coreVersion})` : "browser preview"}
              </div>
            </div>
          </Card>
        ) : (
          <p className="muted">Coming in P2.</p>
        )}
      </main>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
