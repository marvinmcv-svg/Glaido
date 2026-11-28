/** Types and copy for importing from other apps (spec 4.5.4) and dictionary suggestions (4.5.1). */

export type ImportFormat =
  "espanso" | "textExpander" | "alfred" | "raycast" | "wisprFlow" | "superwhisper";

export const FORMATS: { id: ImportFormat; label: string; hint: string }[] = [
  {
    id: "wisprFlow",
    label: "Wispr Flow",
    hint: "A dictionary .csv (word, or misspelling,correction) or a snippets .json",
  },
  {
    id: "superwhisper",
    label: "Superwhisper",
    hint: "A .csv with one word, or original,replacement, per line",
  },
  { id: "raycast", label: "Raycast", hint: "The .json file from Export Snippets" },
  { id: "alfred", label: "Alfred", hint: "A .alfredsnippets collection" },
  { id: "espanso", label: "Espanso", hint: "A match file such as base.yml" },
  { id: "textExpander", label: "TextExpander", hint: "A group exported as .csv" },
];

export type SkipReason =
  | { kind: "tooLong" }
  | { kind: "duplicate" }
  | { kind: "invalid"; detail: string }
  | { kind: "unsupported"; feature: string };

export interface ImportReport {
  format: ImportFormat;
  entries: { word: string; misspelling: string | null }[];
  snippets: { trigger: string; content: string }[];
  skipped: { item: string; reason: SkipReason }[];
  converted: { item: string; change: string }[];
}

export interface Suggestion {
  from: string;
  to: string;
  count: number;
  lastSeen: number;
}

export function reasonText(r: SkipReason): string {
  switch (r.kind) {
    case "tooLong":
      return "Too long for Vocalis";
    case "duplicate":
      return "Already in Vocalis";
    case "invalid":
      return `Couldn't use it: ${r.detail}`;
    case "unsupported":
      return `Uses ${r.feature}, which Vocalis doesn't have`;
  }
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "3 words and 2 snippets" (or "nothing new"). */
export function itemsText(report: ImportReport): string {
  const parts: string[] = [];
  if (report.entries.length) parts.push(plural(report.entries.length, "word", "words"));
  if (report.snippets.length) parts.push(plural(report.snippets.length, "snippet", "snippets"));
  return parts.length ? parts.join(" and ") : "nothing new";
}

export function previewText(report: ImportReport): string {
  const ready = report.entries.length + report.snippets.length;
  const head = ready
    ? `Ready to add ${itemsText(report)}.`
    : "There's nothing new to add from this file.";
  return report.skipped.length
    ? `${head} ${plural(report.skipped.length, "item", "items")} will be left out.`
    : head;
}

export function doneText(report: ImportReport): string {
  return report.entries.length + report.snippets.length
    ? `Added ${itemsText(report)}.`
    : "Nothing was added.";
}

/** Copy for a dictionary suggestion. */
export function suggestionText(s: Suggestion): string {
  return `Add “${s.to}” to your dictionary? You fixed “${s.from}” ${plural(s.count, "time", "times")} lately.`;
}

/** Bytes of a picked file, in the shape Tauri passes to a `Vec<u8>` argument. */
export async function fileBytes(file: Blob): Promise<number[]> {
  return Array.from(new Uint8Array(await file.arrayBuffer()));
}

/** Largest file the importer accepts (matches `vocalis_import::MAX_FILE_BYTES`). */
export const MAX_IMPORT_BYTES = 5 * 1024 * 1024;
