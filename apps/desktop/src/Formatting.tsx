import { invoke, isTauri } from "@tauri-apps/api/core";
import { Button, Card, Switch } from "@vocalis/ui";
import { useCallback, useEffect, useId, useState } from "react";
import {
  addSite,
  type AppRef,
  charCount,
  draftOf,
  emptyDraft,
  type FormattingRule,
  MAX_NAME_CHARS,
  MAX_PROMPT_CHARS,
  pickerSections,
  type PickerItem,
  type Preset,
  type PresetOutcome,
  presetMessage,
  type RuleDraft,
  saveBlocker,
  STYLES,
  targetsLabel,
  toggleApp,
  type UsedApp,
} from "./formattingRules";
import "./formatting.css";
import { createPreviewBackend } from "./formattingPreview";

const preview = createPreviewBackend();

/** Tauri when available; otherwise the in-memory preview backend. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) return invoke<T>(cmd, args);
  return Promise.resolve().then(() => preview(cmd, args) as T);
}

const errorText = (e: unknown) =>
  typeof e === "string" ? e : e instanceof Error ? e.message : "Something went wrong.";

const styleLabel = (s: FormattingRule["style"]) => STYLES.find((x) => x.id === s)?.label ?? s;

/** The Formatting page (spec 4.5.3): All apps, Email, custom rules and presets. */
export function Formatting() {
  const [rules, setRules] = useState<FormattingRule[] | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [editing, setEditing] = useState<string | null>(null); // a rule id, or "new";
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setRules(await call<FormattingRule[]>("rules_get"));
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    call<FormattingRule[]>("rules_get")
      .then(setRules)
      .catch((e: unknown) => setError(errorText(e)));
    call<Preset[]>("formatting_presets")
      .then(setPresets)
      .catch(() => setPresets([]));
  }, [load]);

  const toggle = async (r: FormattingRule, enabled: boolean) => {
    try {
      await call("rule_toggle", { id: r.id, enabled });
      await load();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const addPreset = async (p: Preset) => {
    try {
      const out = await call<PresetOutcome>("rule_add_preset", { presetId: p.id });
      setNotice(presetMessage(p, out));
      setError(null);
      await load();
    } catch (e) {
      setError(errorText(e));
    }
  };

  if (!rules) {
    return error ? (
      <p role="alert" className="error-text">
        {error}
      </p>
    ) : (
      <p className="muted">Loading rules…</p>
    );
  }

  const all = rules.filter((r) => r.kind === "all_apps");
  const email = rules.filter((r) => r.kind === "email");
  const custom = rules.filter((r) => r.kind === "custom");

  const card = (r: FormattingRule) =>
    editing === r.id ? (
      <RuleEditor
        key={r.id}
        rule={r}
        rules={rules}
        onDone={async () => {
          setEditing(null);
          await load();
        }}
        onCancel={() => setEditing(null)}
      />
    ) : (
      <RuleSummary
        key={r.id}
        rule={r}
        onEdit={() => setEditing(r.id)}
        onToggle={(on) => void toggle(r, on)}
      />
    );

  return (
    <div className="formatting">
      <p className="muted lead">
        Choose how dictated text looks in each app. A rule for an app or website builds on All apps:
        its instructions are added after the All apps ones.
      </p>
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
      {notice ? (
        <p role="status" className="notice-text">
          {notice}{" "}
          <button type="button" className="link-button" onClick={() => setNotice(null)}>
            Dismiss
          </button>
        </p>
      ) : null}

      <section aria-labelledby="fmt-all">
        <h2 id="fmt-all">All apps</h2>
        {all.map(card)}
      </section>

      <section aria-labelledby="fmt-email">
        <h2 id="fmt-email">Email</h2>
        {email.map(card)}
      </section>

      <section aria-labelledby="fmt-custom">
        <div className="section-head">
          <h2 id="fmt-custom">Custom rules</h2>
          {editing !== "new" ? (
            <Button variant="ghost" size="sm" onClick={() => setEditing("new")}>
              New rule
            </Button>
          ) : null}
        </div>
        {editing === "new" ? (
          <RuleEditor
            rule={null}
            rules={rules}
            onDone={async () => {
              setEditing(null);
              await load();
            }}
            onCancel={() => setEditing(null)}
          />
        ) : null}
        {custom.length === 0 && editing !== "new" ? (
          <p className="muted">
            No custom rules yet. Make one for an app you dictate into often, or start from a preset
            below.
          </p>
        ) : null}
        {custom.map(card)}
      </section>

      <section aria-labelledby="fmt-presets">
        <h2 id="fmt-presets">Presets</h2>
        <p className="muted">
          One click adds a ready-made rule. Apps and websites that already have a rule are left
          where they are.
        </p>
        <div className="preset-grid">
          {presets.map((p) => (
            <Card key={p.id} className="preset">
              <div className="preset-name">{p.name}</div>
              <div className="muted">{p.summary}</div>
              <div className="chips">
                <span className="chip">{styleLabel(p.style)}</span>
                {p.raw ? <span className="chip">Raw text</span> : null}
                {[...p.apps.map((a) => a.name), ...p.sites].slice(0, 4).map((t) => (
                  <span key={t} className="chip chip-quiet">
                    {t}
                  </span>
                ))}
              </div>
              <Button size="sm" variant="ghost" onClick={() => void addPreset(p)}>
                {p.builtinEmail ? "Apply to Email" : "Add"}
              </Button>
            </Card>
          ))}
        </div>
      </section>
    </div>
  );
}

function RuleSummary({
  rule,
  onEdit,
  onToggle,
}: {
  rule: FormattingRule;
  onEdit: () => void;
  onToggle: (on: boolean) => void;
}) {
  return (
    <Card className="rule" data-enabled={rule.enabled || undefined}>
      <div className="rule-main">
        <div className="rule-title">
          {rule.kind === "custom" ? rule.name : rule.kind === "email" ? "Email" : "Default style"}
        </div>
        <div className="muted">{targetsLabel(rule)}</div>
        <div className="chips">
          <span className="chip">{styleLabel(rule.style)}</span>
          {rule.raw ? <span className="chip">Raw text</span> : null}
          {!rule.raw && rule.prompt ? <span className="chip">Custom prompt</span> : null}
          {!rule.enabled ? <span className="chip">Off</span> : null}
        </div>
      </div>
      <div className="rule-actions">
        {rule.kind !== "all_apps" ? (
          <Switch checked={rule.enabled} onChange={onToggle} label={`${rule.name} rule`} />
        ) : null}
        <Button variant="ghost" size="sm" onClick={onEdit}>
          Edit
        </Button>
      </div>
    </Card>
  );
}

function RuleEditor({
  rule,
  rules,
  onDone,
  onCancel,
}: {
  rule: FormattingRule | null;
  rules: FormattingRule[];
  onDone: () => Promise<void>;
  onCancel: () => void;
}) {
  const kind = rule?.kind ?? "custom";
  const id = rule?.id ?? null;
  const [draft, setDraft] = useState<RuleDraft>(() => (rule ? draftOf(rule) : emptyDraft()));
  const [site, setSite] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const uid = useId();
  const blocker = saveBlocker(draft, kind);
  const count = charCount(draft.prompt.trim());

  const save = async () => {
    // A website typed but not yet added is saved too (no Enter needed).
    const pending = addSite(draft, site, rules, id);
    if (pending.error) {
      setError(pending.error);
      return;
    }
    setBusy(true);
    try {
      await call("rule_save", { id, rule: pending.draft });
      await onDone();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!rule || !window.confirm(`Delete the rule "${rule.name}"?`)) return;
    try {
      await call("rule_delete", { id: rule.id });
      await onDone();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const commitSite = () => {
    const r = addSite(draft, site, rules, id);
    setError(r.error);
    if (!r.error) {
      setDraft(r.draft);
      setSite("");
    }
  };

  return (
    <Card className="rule-editor">
      {kind === "custom" ? (
        <label className="field">
          <span>Name</span>
          <input
            className="input"
            value={draft.name}
            maxLength={MAX_NAME_CHARS}
            placeholder="For example: Code review"
            onChange={(e) => setDraft({ ...draft, name: e.target.value })}
          />
        </label>
      ) : (
        <div className="rule-title">{kind === "email" ? "Email" : "All apps"}</div>
      )}

      {kind !== "all_apps" ? (
        <>
          <AppPicker draft={draft} rules={rules} exceptId={id} onChange={setDraft} />
          <div className="field">
            <label htmlFor={`${uid}-site`}>Websites</label>
            <div className="chips">
              {draft.sites.map((s) => (
                <span key={s} className="chip">
                  {s}{" "}
                  <button
                    type="button"
                    className="chip-remove"
                    aria-label={`Remove ${s}`}
                    onClick={() =>
                      setDraft({ ...draft, sites: draft.sites.filter((x) => x !== s) })
                    }
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>
            <div className="inline">
              <input
                id={`${uid}-site`}
                className="input"
                value={site}
                placeholder="github.com"
                onChange={(e) => setSite(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    commitSite();
                  }
                }}
              />
              <Button variant="ghost" size="sm" onClick={commitSite} disabled={!site.trim()}>
                Add
              </Button>
            </div>
            <div className="muted">
              Matched against the address of the browser tab you dictate into. Subdomains count too.
            </div>
          </div>
        </>
      ) : null}

      <fieldset className="field styles">
        <legend>Style</legend>
        {STYLES.map((s) => (
          <label
            key={s.id}
            className="style-option"
            data-checked={draft.style === s.id || undefined}
          >
            <input
              type="radio"
              name={`${uid}-style`}
              value={s.id}
              checked={draft.style === s.id}
              onChange={() => setDraft({ ...draft, style: s.id })}
            />
            <span className="style-label">{s.label}</span>
            <span className="muted">{s.description}</span>
            <span className="style-example">{s.example}</span>
          </label>
        ))}
      </fieldset>

      <div className="row">
        <div>
          <div>Raw text</div>
          <div className="muted">
            Types exactly what you said, without AI cleanup. The style still applies; dictionary
            spellings are matched less reliably.
          </div>
        </div>
        <Switch
          checked={draft.raw}
          onChange={(raw) => setDraft({ ...draft, raw })}
          label="Raw text"
        />
      </div>

      {!draft.raw ? (
        <div className="field">
          <label htmlFor={`${uid}-prompt`}>Instructions</label>
          <textarea
            id={`${uid}-prompt`}
            className="input prompt"
            rows={4}
            value={draft.prompt}
            placeholder={
              kind === "all_apps"
                ? "For example: use British spelling."
                : "For example: keep messages under two sentences."
            }
            onChange={(e) => setDraft({ ...draft, prompt: e.target.value })}
            aria-describedby={`${uid}-count`}
          />
          <div
            id={`${uid}-count`}
            className="muted counter"
            data-over={count > MAX_PROMPT_CHARS || undefined}
            aria-live="polite"
          >
            {count} / {MAX_PROMPT_CHARS}
          </div>
        </div>
      ) : null}

      {kind !== "all_apps" ? (
        <div className="row">
          <div>Rule on</div>
          <Switch
            checked={draft.enabled}
            onChange={(enabled) => setDraft({ ...draft, enabled })}
            label="Rule on"
          />
        </div>
      ) : null}

      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
      <div className="editor-actions">
        <Button
          onClick={() => void save()}
          disabled={busy || blocker !== null}
          title={blocker ?? undefined}
        >
          Save
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <span className="spacer" />
        {blocker ? <span className="muted">{blocker}</span> : null}
        {kind === "custom" && rule ? (
          <Button variant="ghost" onClick={() => void remove()}>
            Delete rule
          </Button>
        ) : null}
      </div>
    </Card>
  );
}

function AppPicker({
  draft,
  rules,
  exceptId,
  onChange,
}: {
  draft: RuleDraft;
  rules: FormattingRule[];
  exceptId: string | null;
  onChange: (d: RuleDraft) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [manual, setManual] = useState("");
  const [mostUsed, setMostUsed] = useState<AppRef[]>([]);
  const [installed, setInstalled] = useState<AppRef[]>([]);
  const uid = useId();

  useEffect(() => {
    if (!open) return;
    call<UsedApp[]>("apps_most_used")
      .then(setMostUsed)
      .catch(() => setMostUsed([]));
    call<AppRef[]>("apps_installed")
      .then(setInstalled)
      .catch(() => setInstalled([]));
  }, [open]);

  const sections = pickerSections(mostUsed, installed, query, rules, draft, exceptId);
  const list = (title: string, items: PickerItem[]) =>
    items.length ? (
      <div className="picker-section">
        <div className="picker-title">{title}</div>
        <ul>
          {items.map((a) => (
            <li key={a.id}>
              <label
                className="picker-item"
                title={a.owner ? `Already used by "${a.owner}"` : a.id}
                data-owned={a.owner ? true : undefined}
              >
                <input
                  type="checkbox"
                  checked={a.selected}
                  disabled={a.owner !== null}
                  onChange={() => onChange(toggleApp(draft, a))}
                />
                {a.name}
                {a.owner ? <span className="muted"> · in {a.owner}</span> : null}
              </label>
            </li>
          ))}
        </ul>
      </div>
    ) : null;

  const addManual = () => {
    const id = manual.trim();
    if (!id || draft.apps.some((a) => a.id.toLowerCase() === id.toLowerCase())) return;
    onChange(toggleApp(draft, { id, name: id }));
    setManual("");
  };

  return (
    <div className="field">
      <div className="section-head">
        <span>Apps</span>
        <Button variant="ghost" size="sm" onClick={() => setOpen(!open)} aria-expanded={open}>
          {open ? "Done" : "Choose apps"}
        </Button>
      </div>
      <div className="chips">
        {draft.apps.length === 0 ? <span className="muted">No apps chosen.</span> : null}
        {draft.apps.map((a) => (
          <span key={a.id} className="chip" title={a.id}>
            {a.name}{" "}
            <button
              type="button"
              className="chip-remove"
              aria-label={`Remove ${a.name}`}
              onClick={() => onChange(toggleApp(draft, a))}
            >
              ×
            </button>
          </span>
        ))}
      </div>
      {open ? (
        <div className="picker">
          <input
            className="input"
            aria-label="Search apps"
            placeholder="Search apps"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          {list("Most used", sections.mostUsed)}
          {list("All applications", sections.all)}
          {sections.mostUsed.length + sections.all.length === 0 ? (
            <p className="muted">No apps to show. You can add one by its ID below.</p>
          ) : null}
          <div className="inline">
            <label htmlFor={`${uid}-manual`} className="muted">
              App ID
            </label>
            <input
              id={`${uid}-manual`}
              className="input"
              placeholder="com.example.App or App.exe"
              value={manual}
              onChange={(e) => setManual(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addManual();
                }
              }}
            />
            <Button variant="ghost" size="sm" onClick={addManual} disabled={!manual.trim()}>
              Add
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
