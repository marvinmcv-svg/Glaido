import { invoke, isTauri } from "@tauri-apps/api/core";
import { Button, Card, Keycap, StatCard, Switch } from "@vocalis/ui";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import icon from "../assets/app-icon.svg";
import { AccountRow } from "./Account";
import { Activity } from "./Activity";
import "./app.css";
import { currentPlatform, DEFAULT_HOTKEYS } from "./platform";
import { chordKeys, useSettings } from "./settings";
import { Toasts } from "./Toasts";
import { Wizard } from "./Wizard";

type AppInfo = { name: string; version: string; coreVersion: string };
const PAGES = ["Home", "Dictionary", "Snippets", "Formatting", "Settings"] as const;

function App() {
  const [page, setPage] = useState<(typeof PAGES)[number]>("Home");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [sounds, setSounds] = useState(true);
  const { settings, save } = useSettings();
  const [wizard, setWizard] = useState(false);
  const platform = settings?.platform ?? currentPlatform();
  const keys = settings
    ? {
        dictation: chordKeys(settings.hotkeys.dictation),
        handsFree: chordKeys(settings.hotkeys.dictationHandsfree),
      }
    : DEFAULT_HOTKEYS[platform];
  const showWizard = settings !== null && (wizard || !settings.setupCompleted);

  const [engineError, setEngineError] = useState<string | null>(null);
  useEffect(() => {
    if (isTauri()) void invoke<string | null>("engine_error").then(setEngineError);
  }, []);

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
        {engineError ? (
          <p role="alert" className="error-text">
            Dictation is off: {engineError}
          </p>
        ) : null}
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
                <Button
                  variant="ghost"
                  onClick={() => {
                    if (isTauri()) void invoke("toggle_bar");
                  }}
                >
                  Toggle bar
                </Button>
              </div>
            </Card>
            <Activity />
          </>
        ) : page === "Settings" ? (
          <Card>
            <AccountRow />
            <div className="row">
              <div>
                <div>Setup wizard</div>
                <div className="muted">
                  Check permissions, test your microphone, pick languages.
                </div>
              </div>
              <Button variant="ghost" onClick={() => setWizard(true)}>
                Restart wizard
              </Button>
            </div>
            <div className="row">
              <div>
                <div>Paste into the starting app</div>
                <div className="muted">
                  Switched windows while speaking? The text still goes where you started.
                </div>
              </div>
              <Switch
                checked={settings?.pasteIntoStartingApp ?? true}
                onChange={(enabled) =>
                  void save("settings_set_paste_into_starting_app", { enabled })
                }
                label="Paste into the starting app"
              />
            </div>
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
      <Toasts />
      {showWizard && settings ? (
        <Wizard settings={settings} save={save} onDone={() => setWizard(false)} />
      ) : null}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
