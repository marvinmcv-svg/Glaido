import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button, Card, Keycap } from "@vocalis/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import { useAccount } from "./Account";
import { type DayGroup, utcOffsetMinutes } from "./history";
import { defaultLanguage, languages, matchesLanguage } from "./languages";
import { chordKeys, type SettingsView } from "./settings";

type Save = (command: string, args: Record<string, unknown>) => Promise<SettingsView>;
type Props = { settings: SettingsView; save: Save; onDone: () => void };

const STEPS = ["Permissions", "Microphone", "Languages", "First dictation"] as const;
const errorText = (e: unknown) => (typeof e === "string" ? e : "Something went wrong.");

/** First-run setup (spec 4.8): permissions → mic test → languages → first dictation. */
export function Wizard({ settings, save, onDone }: Props) {
  const [step, setStep] = useState(0);
  const finish = async () => {
    await save("settings_set_setup_completed", { completed: true });
    onDone();
  };
  const next = () => (step === STEPS.length - 1 ? void finish() : setStep(step + 1));

  return (
    <div className="wizard" role="dialog" aria-modal="true" aria-labelledby="wizard-title">
      <div className="wizard-panel">
        <ol className="wizard-steps" aria-label="Setup steps">
          {STEPS.map((s, i) => (
            <li key={s} aria-current={i === step ? "step" : undefined} data-done={i < step}>
              {s}
            </li>
          ))}
        </ol>
        <h1 id="wizard-title">{STEPS[step]}</h1>
        {step === 0 ? <PermissionsStep onReady={next} /> : null}
        {step === 1 ? <MicStep settings={settings} save={save} /> : null}
        {step === 2 ? <LanguagesStep settings={settings} save={save} /> : null}
        {step === 3 ? <FirstDictationStep settings={settings} save={save} /> : null}
        <div className="wizard-nav">
          <Button variant="ghost" onClick={() => void finish()}>
            Skip setup
          </Button>
          <span className="spacer" />
          {step > 0 ? (
            <Button variant="ghost" onClick={() => setStep(step - 1)}>
              Back
            </Button>
          ) : null}
          <Button onClick={next}>{step === STEPS.length - 1 ? "Finish" : "Continue"}</Button>
        </div>
      </div>
    </div>
  );
}

type Permissions = {
  platform: "macos" | "windows" | "linux";
  accessibility: "granted" | "missing" | "not_needed";
  microphone: "granted" | "missing" | "unknown";
  wayland: boolean;
};

function PermissionsStep({ onReady }: { onReady: () => void }) {
  const [p, setP] = useState<Permissions | null>(null);
  const [error, setError] = useState<string | null>(null);
  const advanced = useRef(false);

  useEffect(() => {
    if (!isTauri()) return;
    const check = () => void invoke<Permissions>("permissions_status").then(setP);
    check();
    // Re-check while the user is in System Settings; advance once granted.
    const t = setInterval(check, 2000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    if (p?.platform === "macos" && p.accessibility === "granted" && !advanced.current) {
      advanced.current = true;
      setTimeout(onReady, 600);
    }
  }, [p, onReady]);

  const open = (kind: string) =>
    invoke("open_permission_settings", { kind }).catch((e) => setError(errorText(e)));

  if (!p) return <p className="muted">Checking…</p>;
  return (
    <Card>
      {p.platform === "macos" ? (
        <div className="row">
          <div>
            <div>Accessibility {p.accessibility === "granted" ? "✓" : null}</div>
            <div className="muted">
              Lets Vocalis notice your shortcut in any app and paste the text where your cursor is.
            </div>
          </div>
          {p.accessibility === "granted" ? (
            <span className="ok">Allowed</span>
          ) : (
            <Button onClick={() => void open("accessibility")}>Open System Settings</Button>
          )}
        </div>
      ) : null}
      <div className="row">
        <div>
          <div>Microphone</div>
          <div className="muted">
            {p.microphone === "missing"
              ? "No microphone found. Plug one in, or check your sound settings."
              : "Your system asks the first time Vocalis listens. You'll try it in the next step."}
          </div>
        </div>
        {p.platform !== "linux" ? (
          <Button variant="ghost" onClick={() => void open("microphone")}>
            Privacy settings
          </Button>
        ) : null}
      </div>
      {p.wayland ? (
        <p className="notice-text">
          You're on a Wayland session. Some desktops block apps from seeing global shortcuts or
          pasting for you; if dictation doesn't start, see the Linux troubleshooting guide.
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
    </Card>
  );
}

type Mic = { name: string; isDefault: boolean };

function MicStep({ settings, save }: { settings: SettingsView; save: Save }) {
  const [mics, setMics] = useState<Mic[]>([]);
  const [device, setDevice] = useState<string | null>(settings.microphone);
  const [level, setLevel] = useState(0);
  const [peak, setPeak] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isTauri()) void invoke<Mic[]>("mic_list").then(setMics);
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    setError(null);
    setPeak(0);
    const subs = [
      listen<number>("mic-level", (e) => {
        setLevel(e.payload);
        setPeak((p) => Math.max(p, e.payload));
      }),
      listen<string>("mic-test-error", (e) => setError(e.payload)),
    ];
    void invoke("mic_test_start", { device });
    return () => {
      void invoke("mic_test_stop");
      subs.forEach((s) => void s.then((off) => off()));
    };
  }, [device]);

  const choose = (value: string) => {
    const d = value === "" ? null : value;
    setDevice(d);
    void save("settings_set_microphone", { microphone: d });
  };

  return (
    <Card>
      <div className="row">
        <label htmlFor="mic">Microphone</label>
        <select id="mic" value={device ?? ""} onChange={(e) => choose(e.target.value)}>
          <option value="">System default</option>
          {mics.map((m) => (
            <option key={m.name} value={m.name}>
              {m.name}
              {m.isDefault ? " (default)" : ""}
            </option>
          ))}
        </select>
      </div>
      <div className="meter-row">
        <div
          className="meter"
          role="meter"
          aria-label="Input level"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(level * 100)}
        >
          <div className="meter-fill" style={{ width: `${Math.round(level * 100)}%` }} />
        </div>
        <p className="muted" aria-live="polite">
          {error
            ? error
            : peak > 0.35
              ? "We can hear you."
              : "Say a few words. The bar should move as you speak."}
        </p>
      </div>
      <p className="muted">Nothing is recorded or sent during this test.</p>
    </Card>
  );
}

function LanguagesStep({ settings, save }: { settings: SettingsView; save: Save }) {
  const all = useMemo(() => languages(navigator.language), []);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string[]>(() =>
    // The Rust side only sees LANG, which macOS and Windows rarely set: on
    // first run, prefer the UI locale when it disagrees with a plain "en".
    !settings.setupCompleted && settings.languages.join() === "en"
      ? [defaultLanguage()]
      : settings.languages,
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (selected.join() !== settings.languages.join())
      save("settings_set_languages", { languages: selected }).catch((e) => setError(errorText(e)));
  }, [selected, settings.languages, save]);

  const toggle = (code: string) => {
    setError(null);
    if (selected.includes(code)) {
      if (selected.length === 1) return setError("Keep at least one language selected.");
      setSelected(selected.filter((c) => c !== code));
    } else setSelected([...selected, code]);
  };
  const name = (code: string) => all.find((l) => l.code === code)?.name ?? code;
  const shown = all.filter((l) => matchesLanguage(l, query));

  return (
    <Card>
      <p className="muted">
        {selected.length === 1
          ? "One language gives the best accuracy."
          : "With several languages, Vocalis detects which one you're speaking."}
      </p>
      <div className="chips" aria-label="Selected languages">
        {selected.map((c) => (
          <button key={c} className="chip chip-button" onClick={() => toggle(c)}>
            {name(c)} ✕
          </button>
        ))}
      </div>
      <input
        type="search"
        className="input"
        placeholder="Search languages"
        aria-label="Search languages"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <ul className="language-list">
        {shown.map((l) => (
          <li key={l.code}>
            <label>
              <input
                type="checkbox"
                checked={selected.includes(l.code)}
                onChange={() => toggle(l.code)}
              />
              {l.name}
              {l.native !== l.name ? <span className="muted"> · {l.native}</span> : null}
            </label>
          </li>
        ))}
      </ul>
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
    </Card>
  );
}

function FirstDictationStep({ settings, save }: { settings: SettingsView; save: Save }) {
  const { account, signIn, waiting } = useAccount();
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const since = useRef(Date.now());

  useEffect(() => {
    if (!isTauri()) return;
    const off = listen("history-changed", async () => {
      const groups = await invoke<DayGroup[]>("history_list", {
        before: null,
        utcOffsetMinutes: utcOffsetMinutes(),
      });
      const latest = groups[0]?.records[0];
      if (latest && latest.createdAt >= since.current)
        setResult(latest.status === "ok" ? latest.text : null);
    });
    return () => void off.then((f) => f());
  }, []);

  const change = async () => {
    setError(null);
    setRecording(true);
    try {
      const chord = await invoke<string | null>("hotkey_record");
      if (chord)
        await save("settings_set_hotkeys", {
          hotkeys: { ...settings.hotkeys, dictation: chord },
        });
    } catch (e) {
      setError(errorText(e));
    } finally {
      setRecording(false);
    }
  };

  return (
    <Card>
      {!account ? (
        <div className="row">
          <div>
            <div>Sign in first</div>
            <div className="muted">Dictation needs your Vocalis account.</div>
          </div>
          <Button onClick={() => void signIn()}>
            {waiting ? "Open browser again" : "Sign in"}
          </Button>
        </div>
      ) : null}
      <div className="row">
        <div>
          <div>Hold to dictate</div>
          <div className="muted">
            {recording
              ? "Press the keys you want, then let go. Esc cancels."
              : "Hold the keys, speak, and let go."}
          </div>
        </div>
        <span className="hotkey">
          <Keycap keys={chordKeys(settings.hotkeys.dictation)} platform={settings.platform} />
          <Button variant="ghost" disabled={recording} onClick={() => void change()}>
            {recording ? "Listening…" : "Change"}
          </Button>
        </span>
      </div>
      <label className="try-it">
        <span className="muted">Click in the box, then try it: “This is my first dictation.”</span>
        <textarea rows={3} placeholder="Your words appear here" />
      </label>
      <p aria-live="polite" className={result ? "ok" : "muted"}>
        {result ? `It worked: “${result}”` : "Waiting for your first dictation…"}
      </p>
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
    </Card>
  );
}
