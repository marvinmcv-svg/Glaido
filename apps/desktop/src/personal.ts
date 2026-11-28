/**
 * Dictionary and snippet types and pure helpers (spec 4.5.1, 4.5.2).
 * Mirrors `vocalis_store::{DictionaryEntry, SnippetEntry}` and the rules in
 * `vocalis_core::{dictionary, snippets}`; the backend re-checks everything.
 */

export type DictionaryEntry = {
  id: string;
  word: string;
  misspelling: string | null;
  createdAt: number;
  updatedAt: number;
};

export type SnippetEntry = {
  id: string;
  trigger: string;
  content: string;
  createdAt: number;
  updatedAt: number;
};

export const MAX_ENTRY_CHARS = 50;
export const MAX_TRIGGER_CHARS = 200;
export const MAX_CONTENT_CHARS = 10_000;

export const TAGS = [
  { tag: "{clipboard}", label: "Clipboard", hint: "Whatever text you last copied" },
  { tag: "{datetime}", label: "Date and time", hint: "Now, as 2026-11-28 14:05" },
  { tag: "{timezone}", label: "Time zone", hint: "Your time zone, such as Europe/Berlin" },
] as const;

export const DUPLICATE_ENTRY = "Duplicate entry";

/** Characters as the backend counts them (Unicode scalars, ends trimmed). */
export function charCount(s: string): number {
  return [...s.trim()].length;
}

export function hasTag(s: string): boolean {
  return TAGS.some((t) => s.includes(t.tag));
}

const key = (word: string, misspelling: string | null) =>
  `${word.trim().toLowerCase()}\u0000${(misspelling ?? "").trim().toLowerCase()}`;

export type EntryDraft = { kind: "vocabulary" | "correction"; word: string; misspelling: string };

/** The misspelling to send: `null` for vocabulary or when left blank. */
export function draftMisspelling(d: EntryDraft): string | null {
  return d.kind === "correction" && d.misspelling.trim() ? d.misspelling : null;
}

/** Why a draft can't be saved yet, or `null` when it can. */
export function entryError(
  d: EntryDraft,
  entries: DictionaryEntry[],
  editingId: string | null,
): string | null {
  if (!d.word.trim())
    return d.kind === "correction" ? "Type the correct spelling." : "Type a word.";
  if (charCount(d.word) > MAX_ENTRY_CHARS) return "That's more than 50 characters.";
  const misspelling = draftMisspelling(d);
  if (misspelling && charCount(misspelling) > MAX_ENTRY_CHARS)
    return "That's more than 50 characters.";
  if (hasTag(d.word) && !misspelling) return "A tag needs a phrase to replace. Fill in “You say”.";
  const k = key(d.word, misspelling);
  if (entries.some((e) => e.id !== editingId && key(e.word, e.misspelling) === k))
    return DUPLICATE_ENTRY;
  return null;
}

export function draftFor(e: DictionaryEntry | null): EntryDraft {
  if (!e) return { kind: "vocabulary", word: "", misspelling: "" };
  return e.misspelling
    ? { kind: "correction", word: e.word, misspelling: e.misspelling }
    : { kind: "vocabulary", word: e.word, misspelling: "" };
}

const includes = (s: string | null, q: string) => (s ?? "").toLowerCase().includes(q);

/** Search covers both sides of each entry. */
export function filterEntries(entries: DictionaryEntry[], query: string): DictionaryEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return entries;
  return entries.filter((e) => includes(e.word, q) || includes(e.misspelling, q));
}

/** Search covers trigger and content. */
export function filterSnippets(snippets: SnippetEntry[], query: string): SnippetEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return snippets;
  return snippets.filter((s) => includes(s.trigger, q) || includes(s.content, q));
}

/** Put `text` in place of the selection; returns the new value and caret. */
export function insertAt(
  value: string,
  text: string,
  start: number | null,
  end: number | null,
): { value: string; caret: number } {
  const s = Math.min(start ?? value.length, value.length);
  const e = Math.max(s, Math.min(end ?? s, value.length));
  return { value: value.slice(0, s) + text + value.slice(e), caret: s + text.length };
}

const EDGE_PUNCT = new Set([..."…“”‘’¡¿«»。、，！？"]);
const isEdge = (c: string) => /\s/.test(c) || /[!-/:-@[-`{-~]/.test(c) || EDGE_PUNCT.has(c);

/** Trim, drop punctuation at both ends, lowercase: how triggers are compared. */
export function normalizePhrase(s: string): string {
  const chars = [...s];
  let a = 0;
  let b = chars.length;
  while (a < b && isEdge(chars[a]!)) a++;
  while (b > a && isEdge(chars[b - 1]!)) b--;
  return chars.slice(a, b).join("").toLowerCase();
}

/** Another live snippet with the same trigger, if any. */
export function sharedTrigger(
  trigger: string,
  snippets: SnippetEntry[],
  editingId: string | null,
): SnippetEntry | null {
  const t = normalizePhrase(trigger);
  if (!t) return null;
  return snippets.find((s) => s.id !== editingId && normalizePhrase(s.trigger) === t) ?? null;
}

/**
 * Snippets that never fire because an earlier one (the list is in match
 * order, oldest first) has the same trigger.
 */
export function shadowedIds(snippets: SnippetEntry[]): Set<string> {
  const seen = new Set<string>();
  const out = new Set<string>();
  for (const s of snippets) {
    const t = normalizePhrase(s.trigger);
    if (seen.has(t)) out.add(s.id);
    else seen.add(t);
  }
  return out;
}

export type SnippetDraft = { trigger: string; content: string };

export function snippetError(d: SnippetDraft): string | null {
  if (!normalizePhrase(d.trigger)) return "Type a trigger phrase.";
  if (charCount(d.trigger) > MAX_TRIGGER_CHARS) return "Keep the trigger under 200 characters.";
  if (!d.content.trim()) return "Type the text to insert.";
  if ([...d.content].length > MAX_CONTENT_CHARS) return "Keep the text under 10,000 characters.";
  return null;
}

/** A short one-line preview of multi-line content. */
export function preview(content: string, max = 120): string {
  const line = content.replace(/\s+/g, " ").trim();
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/** Commands reject with a short sentence; anything else gets a generic one. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong.";
}

/**
 * Browser preview stand-in for the desktop commands: keeps entries in memory
 * with the same checks, so the pages can be tried without the app.
 */
export function previewBackend() {
  let dictionary: DictionaryEntry[] = [];
  let snippets: SnippetEntry[] = [];
  let n = 0;
  const id = () => `preview-${++n}`;
  type Args = Record<string, unknown>;
  const str = (a: Args, k: string) => {
    const v = a[k];
    return typeof v === "string" ? v : "";
  };

  const entry = (a: Args, editing: string | null): DictionaryEntry => {
    const m = typeof a.misspelling === "string" ? a.misspelling : null;
    const d: EntryDraft = {
      kind: m ? "correction" : "vocabulary",
      word: str(a, "word"),
      misspelling: m ?? "",
    };
    const err = entryError(d, dictionary, editing);
    if (err) throw new Error(err);
    const now = Date.now();
    return {
      id: editing ?? id(),
      word: d.word.trim(),
      misspelling: m?.trim() || null,
      createdAt: now,
      updatedAt: now,
    };
  };
  const snippet = (a: Args, editing: string | null): SnippetEntry => {
    const d = { trigger: str(a, "trigger").trim(), content: str(a, "content") };
    const err = snippetError(d);
    if (err) throw new Error(err);
    const now = Date.now();
    return { id: editing ?? id(), ...d, createdAt: now, updatedAt: now };
  };
  const replace = <T extends { id: string; createdAt: number }>(list: T[], next: T) =>
    list.map((x) => (x.id === next.id ? { ...next, createdAt: x.createdAt } : x));

  return async function call<T>(cmd: string, a: Args = {}): Promise<T> {
    await Promise.resolve();
    const r = ((): unknown => {
      switch (cmd) {
        case "dictionary_list":
          return [...dictionary];
        case "dictionary_add": {
          const e = entry(a, null);
          dictionary = [e, ...dictionary];
          return e;
        }
        case "dictionary_update": {
          const e = entry(a, str(a, "id"));
          dictionary = replace(dictionary, e);
          return e;
        }
        case "dictionary_delete":
          dictionary = dictionary.filter((e) => e.id !== a.id);
          return null;
        case "dictionary_delete_all": {
          const count = dictionary.length;
          dictionary = [];
          return count;
        }
        case "snippets_list":
          return [...snippets];
        case "snippets_add": {
          const s = snippet(a, null);
          snippets = [...snippets, s];
          return s;
        }
        case "snippets_update": {
          const s = snippet(a, str(a, "id"));
          snippets = replace(snippets, s);
          return s;
        }
        case "snippets_delete":
          snippets = snippets.filter((s) => s.id !== a.id);
          return null;
        default:
          throw new Error(`Unknown command ${cmd}`);
      }
    })();
    return r as T;
  };
}
