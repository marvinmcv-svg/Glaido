import { Button, Card } from "@vocalis/ui";
import { type FormEvent, type KeyboardEvent, useRef, useState } from "react";
import {
  charCount,
  type DictionaryEntry,
  draftFor,
  draftMisspelling,
  DUPLICATE_ENTRY,
  entryError,
  type EntryDraft,
  filterEntries,
  hasTag,
  MAX_ENTRY_CHARS,
} from "./personal";
import { Counter, DeleteControl, TagPicker } from "./PersonalParts";
import { usePersonal } from "./usePersonal";
import { ImportDialog, DictionarySuggestions } from "./Import";

/** The Dictionary page (spec 4.5.1): vocabulary words and corrections. */
export function Dictionary() {
  const { items, error, setError, run, preview } = usePersonal<DictionaryEntry>("dictionary_list");
  const entries = items ?? [];
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<string | null>(null); // an id, or "new"
  const [confirmAll, setConfirmAll] = useState(false);
  const [importing, setImporting] = useState(false);
  const shown = filterEntries(entries, query);

  const save = async (d: EntryDraft, id: string | null) => {
    const args = { word: d.word, misspelling: draftMisspelling(d) };
    const err = await run(id ? "dictionary_update" : "dictionary_add", id ? { id, ...args } : args);
    if (!err) setEditing(null);
    return err;
  };
  const remove = async (id: string) => {
    if (editing === id) setEditing(null);
    setError(await run("dictionary_delete", { id }));
  };
  const removeAll = async () => {
    setConfirmAll(false);
    setEditing(null);
    setError(await run("dictionary_delete_all"));
  };

  const editor = (entry: DictionaryEntry | null) => (
    <EntryEditor
      entry={entry}
      entries={entries}
      onSave={(d) => save(d, entry?.id ?? null)}
      onCancel={() => setEditing(null)}
    />
  );

  return (
    <section className="personal" aria-label="Dictionary">
      <p className="muted lead">
        Teach Vocalis the names, jargon and brands you use, so they come out spelled right. A
        correction swaps what you say for what you want written, and can drop in your clipboard, the
        date or your time zone.
      </p>
      <div className="personal-head">
        <input
          type="search"
          className="input"
          aria-label="Search your dictionary"
          placeholder="Search words and corrections"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <Button onClick={() => setEditing("new")} disabled={editing === "new"}>
          Add word
        </Button>
        <Button variant="ghost" onClick={() => setImporting(true)}>
          Import
        </Button>
        {entries.length > 0 ? (
          <Button variant="ghost" onClick={() => setConfirmAll(true)} disabled={confirmAll}>
            Delete all words
          </Button>
        ) : null}
      </div>
      {preview ? (
        <p className="muted">Browser preview: entries are kept only until you reload.</p>
      ) : null}
      {confirmAll ? (
        <div className="confirm-bar" role="alertdialog" aria-label="Delete all words">
          <span>
            Delete all {entries.length} {entries.length === 1 ? "entry" : "entries"}? This can’t be
            undone.
          </span>
          <Button variant="attention" onClick={() => void removeAll()}>
            Delete all
          </Button>
          <Button variant="ghost" onClick={() => setConfirmAll(false)}>
            Cancel
          </Button>
        </div>
      ) : null}
      {importing ? <ImportDialog onClose={() => setImporting(false)} /> : null}
      <DictionarySuggestions />
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
      {editing === "new" ? <Card className="editor-card">{editor(null)}</Card> : null}
      <Card>
        {items === null ? (
          <p className="muted">Loading…</p>
        ) : shown.length === 0 ? (
          <p className="muted">
            {entries.length === 0
              ? "Your dictionary is empty. Add a word Vocalis keeps getting wrong."
              : `Nothing matches “${query.trim()}”.`}
          </p>
        ) : (
          <ul className="items">
            {shown.map((e) =>
              editing === e.id ? (
                <li key={e.id} className="item editing">
                  {editor(e)}
                </li>
              ) : (
                <li key={e.id} className="item">
                  <button
                    type="button"
                    className="item-main"
                    onClick={() => setEditing(e.id)}
                    aria-label={`Edit ${e.misspelling ? `${e.misspelling} to ${e.word}` : e.word}`}
                  >
                    {e.misspelling ? (
                      <>
                        <span className="muted">{e.misspelling}</span>
                        <span aria-hidden="true" className="arrow">
                          →
                        </span>
                        <span>{e.word}</span>
                        <span className="chip">{hasTag(e.word) ? "Tag" : "Correction"}</span>
                      </>
                    ) : (
                      <span>{e.word}</span>
                    )}
                  </button>
                  <DeleteControl
                    label={`Delete ${e.word}`}
                    allowShift
                    onDelete={() => void remove(e.id)}
                  />
                </li>
              ),
            )}
          </ul>
        )}
      </Card>
    </section>
  );
}

function EntryEditor({
  entry,
  entries,
  onSave,
  onCancel,
}: {
  entry: DictionaryEntry | null;
  entries: DictionaryEntry[];
  onSave: (d: EntryDraft) => Promise<string | null>;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState<EntryDraft>(() => draftFor(entry));
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const wordRef = useRef<HTMLInputElement>(null);
  const problem = entryError(draft, entries, entry?.id ?? null);
  const touched = draft.word.trim() !== "" || draft.misspelling.trim() !== "";
  const shownProblem =
    problem && (touched || problem === DUPLICATE_ENTRY) ? problem : (saveError ?? null);
  const correction = draft.kind === "correction";

  const set = (patch: Partial<EntryDraft>) => {
    setSaveError(null);
    setDraft((d) => ({ ...d, ...patch }));
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") onCancel();
  };
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (problem || saving) return;
    setSaving(true);
    setSaveError(await onSave(draft));
    setSaving(false);
  };

  return (
    <form className="editor" onSubmit={(e) => void submit(e)}>
      <div className="segmented" role="radiogroup" aria-label="Entry type">
        {(["vocabulary", "correction"] as const).map((k) => (
          <button
            key={k}
            type="button"
            role="radio"
            aria-checked={draft.kind === k}
            onClick={() => set({ kind: k })}
          >
            {k === "vocabulary" ? "Vocabulary" : "Correction"}
          </button>
        ))}
      </div>
      <p className="muted">
        {correction
          ? "When Vocalis hears the phrase on the left, it writes the text on the right."
          : "A word Vocalis should recognize and spell exactly like this."}
      </p>
      <div className={correction ? "editor-fields two" : "editor-fields"}>
        {correction ? (
          <label className="field">
            <span className="field-label">
              You say <Counter count={charCount(draft.misspelling)} max={MAX_ENTRY_CHARS} />
            </span>
            <input
              className="input"
              value={draft.misspelling}
              placeholder="postgres"
              onChange={(e) => set({ misspelling: e.target.value })}
              onKeyDown={keys}
            />
          </label>
        ) : null}
        <label className="field">
          <span className="field-label">
            {correction ? "Write it as" : "Word"}{" "}
            <Counter count={charCount(draft.word)} max={MAX_ENTRY_CHARS} />
          </span>
          <input
            ref={wordRef}
            className="input"
            value={draft.word}
            placeholder={correction ? "PostgreSQL" : "Vocalis"}
            onChange={(e) => set({ word: e.target.value })}
            onKeyDown={keys}
          />
        </label>
      </div>
      {correction ? (
        <TagPicker target={wordRef} value={draft.word} onChange={(word) => set({ word })} />
      ) : null}
      {shownProblem ? (
        <p role="alert" className="error-text">
          {shownProblem}
        </p>
      ) : null}
      <div className="editor-actions">
        <Button type="submit" disabled={problem !== null || saving}>
          {entry ? "Save" : "Add"}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
