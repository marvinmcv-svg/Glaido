import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@vocalis/ui";
import { useEffect, useId, useState } from "react";
import "./import.css";
import {
  doneText,
  fileBytes,
  FORMATS,
  type ImportFormat,
  type ImportReport,
  MAX_IMPORT_BYTES,
  previewText,
  reasonText,
  type Suggestion,
  suggestionText,
} from "./importer";

const errorText = (e: unknown) => (typeof e === "string" ? e : "Something went wrong.");

type Picked = { name: string; data: number[] };

/**
 * Import dictionary words and snippets from another app (spec 4.5.4).
 *
 * Mount it from any page, e.g. the Dictionary or Snippets page:
 *
 *   const [importing, setImporting] = useState(false);
 *   <Button onClick={() => setImporting(true)}>Import…</Button>
 *   {importing && <ImportDialog onClose={() => setImporting(false)} />}
 *
 * The backend emits `dictionary-changed` / `snippets-changed` after saving,
 * so pages that listen for those refresh themselves; `onImported` is there too.
 */
export function ImportDialog({
  onClose,
  onImported,
}: {
  onClose: () => void;
  onImported?: (report: ImportReport) => void;
}) {
  const titleId = useId();
  const fileId = useId();
  const formatId = useId();
  const [format, setFormat] = useState<ImportFormat | "">("");
  const [file, setFile] = useState<Picked | null>(null);
  const [preview, setPreview] = useState<ImportReport | null>(null);
  const [done, setDone] = useState<ImportReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const args = (f: Picked, fmt: ImportFormat | "") => ({
    path: null,
    name: f.name,
    data: f.data,
    format: fmt || null,
  });

  const runPreview = async (f: Picked, fmt: ImportFormat | "") => {
    if (!isTauri()) return;
    setBusy(true);
    setPreview(null);
    try {
      setPreview(await invoke<ImportReport>("import_preview", args(f, fmt)));
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const pick = async (input: HTMLInputElement) => {
    const chosen = input.files?.[0];
    setDone(null);
    if (!chosen) return;
    if (chosen.size > MAX_IMPORT_BYTES) {
      setError("That file is larger than 5 MB.");
      return;
    }
    const picked = { name: chosen.name, data: await fileBytes(chosen) };
    setFile(picked);
    await runPreview(picked, format);
  };

  const changeFormat = async (fmt: ImportFormat | "") => {
    setFormat(fmt);
    if (file) await runPreview(file, fmt);
  };

  const apply = async () => {
    if (!file || !preview) return;
    setBusy(true);
    try {
      const result = await invoke<ImportReport>("import_apply", args(file, preview.format));
      setDone(result);
      setPreview(null);
      setError(null);
      onImported?.(result);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const hint = FORMATS.find((f) => f.id === format)?.hint;
  const ready = preview ? preview.entries.length + preview.snippets.length : 0;

  return (
    <div className="import-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
      <div className="import-panel">
        <h2 id={titleId}>Bring your words and snippets</h2>
        <p className="muted">
          Pick a file you exported from another app. Nothing is saved until you press Import.
        </p>
        <div className="import-field">
          <label htmlFor={formatId}>Exported from</label>
          <select
            id={formatId}
            value={format}
            onChange={(e) => void changeFormat(e.target.value as ImportFormat | "")}
          >
            <option value="">Work it out from the file</option>
            {FORMATS.map((f) => (
              <option key={f.id} value={f.id}>
                {f.label}
              </option>
            ))}
          </select>
          {hint && <span className="muted">{hint}</span>}
        </div>
        <div className="import-field">
          <label htmlFor={fileId}>File</label>
          <input
            id={fileId}
            type="file"
            accept=".csv,.txt,.json,.yml,.yaml,.alfredsnippets"
            onChange={(e) => void pick(e.currentTarget)}
          />
        </div>
        {error && (
          <p className="error-text" role="alert">
            {error}
          </p>
        )}
        {busy && <p className="muted">Reading…</p>}
        {preview && (
          <>
            <p>{previewText(preview)}</p>
            <ReportList report={preview} />
          </>
        )}
        {done && (
          <>
            <p role="status">{doneText(done)}</p>
            <ReportList report={{ ...done, converted: [] }} />
          </>
        )}
        <div className="import-actions">
          <Button variant="ghost" onClick={onClose}>
            {done ? "Done" : "Cancel"}
          </Button>
          {!done && (
            <Button disabled={!ready || busy} onClick={() => void apply()}>
              {ready ? `Import ${ready}` : "Import"}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

function ReportList({ report }: { report: ImportReport }) {
  if (!report.skipped.length && !report.converted.length) return null;
  return (
    <ul className="import-list" aria-label="Details">
      {report.skipped.map((s, i) => (
        <li key={`s${i}`}>
          <span>{s.item}</span>
          <span className="muted">{reasonText(s.reason)}</span>
        </li>
      ))}
      {report.converted.map((c, i) => (
        <li key={`c${i}`}>
          <span>{c.item}</span>
          <span className="muted">{c.change}</span>
        </li>
      ))}
    </ul>
  );
}

/**
 * Dictionary suggestions with Add and Dismiss (spec 4.5.1). Mount it on the
 * Dictionary page or Home; it refreshes on `suggestions-changed`.
 */
export function DictionarySuggestions() {
  const [items, setItems] = useState<Suggestion[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    const load = () =>
      invoke<Suggestion[]>("suggestion_list")
        .then(setItems)
        .catch((e) => setError(errorText(e)));
    void load();
    const offs = [
      listen("suggestions-changed", () => void load()),
      listen("dictionary-changed", () => void load()),
    ];
    return () => offs.forEach((off) => void off.then((f) => f()));
  }, []);

  const act = async (s: Suggestion, command: "suggestion_accept" | "suggestion_dismiss") => {
    try {
      await invoke(command, { from: s.from, to: s.to });
      setItems((list) => list.filter((x) => x !== s));
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  };

  if (!items.length && !error) return null;
  return (
    <section aria-label="Dictionary suggestions">
      {error && <p className="error-text">{error}</p>}
      {items.map((s) => (
        <div key={`${s.from}>${s.to}`} className="suggestion">
          <p>{suggestionText(s)}</p>
          <Button size="sm" onClick={() => void act(s, "suggestion_accept")}>
            Add
          </Button>
          <Button size="sm" variant="ghost" onClick={() => void act(s, "suggestion_dismiss")}>
            Don't suggest
          </Button>
        </div>
      ))}
    </section>
  );
}
