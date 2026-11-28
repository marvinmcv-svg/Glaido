import { Button, Card } from "@vocalis/ui";
import { type FormEvent, type KeyboardEvent, useRef, useState } from "react";
import {
  charCount,
  filterSnippets,
  MAX_TRIGGER_CHARS,
  preview,
  shadowedIds,
  sharedTrigger,
  type SnippetDraft,
  type SnippetEntry,
  snippetError,
} from "./personal";
import { Counter, DeleteControl, TagPicker } from "./PersonalParts";
import { usePersonal } from "./usePersonal";

/** The Snippets page (spec 4.5.2): say a trigger, get saved text. */
export function Snippets() {
  const {
    items,
    error,
    setError,
    run,
    preview: isPreview,
  } = usePersonal<SnippetEntry>("snippets_list");
  const snippets = items ?? [];
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<string | null>(null); // an id, or "new"
  const shown = filterSnippets(snippets, query);

  const save = async (d: SnippetDraft, id: string | null) => {
    const err = await run(id ? "snippets_update" : "snippets_add", id ? { id, ...d } : d);
    if (!err) setEditing(null);
    return err;
  };
  const remove = async (id: string) => {
    if (editing === id) setEditing(null);
    setError(await run("snippets_delete", { id }));
  };
  const shadowed = shadowedIds(snippets);

  const editor = (s: SnippetEntry | null) => (
    <SnippetEditor
      snippet={s}
      snippets={snippets}
      onSave={(d) => save(d, s?.id ?? null)}
      onCancel={() => setEditing(null)}
    />
  );

  return (
    <section className="personal" aria-label="Snippets">
      <p className="muted lead">
        Say a trigger phrase on its own and Vocalis types the saved text instead: an address, a
        sign-off, a link you share often. Pick short, distinct triggers you would never dictate by
        themselves, like “my postal address”.
      </p>
      <div className="personal-head">
        <input
          type="search"
          className="input"
          aria-label="Search your snippets"
          placeholder="Search triggers and text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <Button onClick={() => setEditing("new")} disabled={editing === "new"}>
          Add snippet
        </Button>
      </div>
      {isPreview ? (
        <p className="muted">Browser preview: snippets are kept only until you reload.</p>
      ) : null}
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
            {snippets.length === 0
              ? "No snippets yet. Add one for text you type again and again."
              : `Nothing matches “${query.trim()}”.`}
          </p>
        ) : (
          <ul className="items">
            {shown.map((s) =>
              editing === s.id ? (
                <li key={s.id} className="item editing">
                  {editor(s)}
                </li>
              ) : (
                <li key={s.id} className="item">
                  <button
                    type="button"
                    className="item-main snippet"
                    onClick={() => setEditing(s.id)}
                    aria-label={`Edit snippet ${s.trigger}`}
                  >
                    <span className="snippet-trigger">{s.trigger}</span>
                    <span className="muted snippet-preview">{preview(s.content)}</span>
                    {shadowed.has(s.id) ? (
                      <span
                        className="chip"
                        title="An earlier snippet has the same trigger, so that one is used."
                      >
                        Same trigger
                      </span>
                    ) : null}
                  </button>
                  <DeleteControl
                    label={`Delete snippet ${s.trigger}`}
                    allowShift={false}
                    onDelete={() => void remove(s.id)}
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

function SnippetEditor({
  snippet,
  snippets,
  onSave,
  onCancel,
}: {
  snippet: SnippetEntry | null;
  snippets: SnippetEntry[];
  onSave: (d: SnippetDraft) => Promise<string | null>;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState<SnippetDraft>(() => ({
    trigger: snippet?.trigger ?? "",
    content: snippet?.content ?? "",
  }));
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const contentRef = useRef<HTMLTextAreaElement>(null);
  const problem = snippetError(draft);
  const touched = draft.trigger.trim() !== "" && draft.content.trim() !== "";
  const twin = sharedTrigger(draft.trigger, snippets, snippet?.id ?? null);
  // The list is oldest first; whoever was saved first keeps the trigger.
  const twinWins = twin !== null && (!snippet || twin.createdAt <= snippet.createdAt);

  const set = (patch: Partial<SnippetDraft>) => {
    setSaveError(null);
    setDraft((d) => ({ ...d, ...patch }));
  };
  // Escape cancels; Ctrl/Cmd+Enter saves (plain Enter is a new line in the text).
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") onCancel();
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void submit();
  };
  const submit = async (e?: FormEvent) => {
    e?.preventDefault();
    if (problem || saving) return;
    setSaving(true);
    setSaveError(await onSave(draft));
    setSaving(false);
  };

  return (
    <form className="editor" onSubmit={(e) => void submit(e)}>
      <label className="field">
        <span className="field-label">
          When you say <Counter count={charCount(draft.trigger)} max={MAX_TRIGGER_CHARS} />
        </span>
        <input
          className="input"
          value={draft.trigger}
          placeholder="my postal address"
          onChange={(e) => set({ trigger: e.target.value })}
          onKeyDown={keys}
        />
      </label>
      {twin ? (
        <p className="notice-text" role="status">
          {twinWins
            ? `Another snippet already uses this trigger. The one saved first wins, so this one won’t be used.`
            : `Another snippet uses this trigger too. This one was saved first, so it wins.`}
        </p>
      ) : null}
      <label className="field">
        <span className="field-label">Vocalis types</span>
        <textarea
          ref={contentRef}
          className="input"
          rows={5}
          value={draft.content}
          placeholder={"12 Example Road\nSpringfield"}
          onChange={(e) => set({ content: e.target.value })}
          onKeyDown={keys}
        />
      </label>
      <TagPicker
        target={contentRef}
        value={draft.content}
        onChange={(content) => set({ content })}
      />
      {(touched && problem) || saveError ? (
        <p role="alert" className="error-text">
          {saveError ?? problem}
        </p>
      ) : null}
      <div className="editor-actions">
        <Button type="submit" disabled={problem !== null || saving}>
          {snippet ? "Save" : "Add"}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
