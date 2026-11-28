/** Command Palette model (spec 4.6): rows, filtering, synonyms and keyboard selection. Pure. */

import type { HistoryRecord } from "./history";
import { snippet } from "./history";

/** Transcript search needs this many characters and shows at most this many results. */
export const MIN_TRANSCRIPT_QUERY = 2;
export const TRANSCRIPT_LIMIT = 15;

export type PaletteAction =
  | { type: "page"; page: string; anchor?: string }
  | { type: "theme" }
  | { type: "wizard" }
  | { type: "record"; id: string; query: string };

export type PaletteGroup = "Go to" | "Add" | "Settings" | "Transcripts";

export type PaletteRow = {
  id: string;
  group: PaletteGroup;
  label: string;
  /** Extra words that find this row, synonyms included. */
  keywords: string[];
  hint?: string;
  action: PaletteAction;
};

/** Search words that point at another page's name (only used when that row exists). */
export const SYNONYMS: Record<string, string> = {
  personalization: "Formatting",
  personalisation: "Formatting",
  style: "Formatting",
  tools: "Built-in tools",
  mcp: "MCP servers",
  vocabulary: "Dictionary",
  words: "Dictionary",
  stats: "Insights",
  charts: "Insights",
  history: "Home",
  activity: "Home",
  preferences: "Settings",
};

export type PaletteOptions = {
  /** Pages in the nav; navigation and quick-add rows exist only for these. */
  pages: readonly string[];
  /** Settings shortcut label, e.g. "⌘," or "Ctrl+,". */
  settingsShortcut: string;
  theme: "light" | "dark";
};

/** The static rows for the pages and settings this build has. */
export function buildRows({ pages, settingsShortcut, theme }: PaletteOptions): PaletteRow[] {
  const has = (p: string) => pages.includes(p);
  const synonymsFor = (label: string) =>
    Object.entries(SYNONYMS)
      .filter(([, target]) => target === label)
      .map(([word]) => word);
  const rows: PaletteRow[] = [];
  for (const page of ["Home", "Formatting", "Dictionary", "Snippets", "Insights"]) {
    if (has(page))
      rows.push({
        id: `go:${page}`,
        group: "Go to",
        label: page,
        keywords: synonymsFor(page),
        action: { type: "page", page },
      });
  }
  const adds: [string, string, string[]][] = [
    ["Add words", "Dictionary", ["new word", "correction", "vocabulary"]],
    ["Add snippet", "Snippets", ["new snippet", "shortcut", "text expansion"]],
    ["Add formatting rule", "Formatting", ["new rule", "style", "app rule"]],
  ];
  for (const [label, page, keywords] of adds) {
    if (has(page))
      rows.push({
        id: `add:${page}`,
        group: "Add",
        label,
        keywords,
        action: { type: "page", page, anchor: "add" },
      });
  }
  if (has("Settings")) {
    rows.push(
      {
        id: "settings:general",
        group: "Settings",
        label: "General",
        keywords: ["settings", ...synonymsFor("Settings"), "sounds", "paste", "version"],
        hint: settingsShortcut,
        action: { type: "page", page: "Settings" },
      },
      {
        id: "settings:account",
        group: "Settings",
        label: "Account",
        keywords: ["sign in", "sign out", "login", "profile"],
        action: { type: "page", page: "Settings", anchor: "account" },
      },
      {
        id: "settings:wizard",
        group: "Settings",
        label: "Setup wizard",
        keywords: [
          "hotkeys",
          "shortcut",
          "microphone",
          "mic",
          "languages",
          "permissions",
          "dictation",
        ],
        action: { type: "wizard" },
      },
      {
        id: "settings:clear",
        group: "Settings",
        label: "Clear history",
        keywords: ["delete", "erase", "wipe", "privacy"],
        action: { type: "page", page: "Settings", anchor: "clear-history" },
      },
    );
  }
  rows.push({
    id: "settings:theme",
    group: "Settings",
    label: theme === "dark" ? "Switch to light theme" : "Switch to dark theme",
    keywords: ["theme", "appearance", "dark mode", "light mode", "colors"],
    action: { type: "theme" },
  });
  return rows;
}

const norm = (s: string) => s.toLowerCase().normalize("NFKD").replace(/\p{M}/gu, "").trim();

/** 0 = label starts with the query, 1 = a label word starts with it, 2 = keyword or substring, -1 = no match. */
function score(row: PaletteRow, q: string): number {
  const label = norm(row.label);
  if (label.startsWith(q)) return 0;
  if (label.split(/\s+/).some((w) => w.startsWith(q))) return 1;
  if (label.includes(q) || row.keywords.some((k) => norm(k).startsWith(q) || norm(k).includes(q)))
    return 2;
  return -1;
}

/** Rows matching `query`, best first; group order is kept within each score. All rows for an empty query. */
export function filterRows(rows: PaletteRow[], query: string): PaletteRow[] {
  const q = norm(query);
  if (!q) return rows;
  return rows
    .map((row, i) => ({ row, i, s: score(row, q) }))
    .filter((x) => x.s >= 0)
    .sort((a, b) => a.s - b.s || a.i - b.i)
    .map((x) => x.row);
}

export function canSearchTranscripts(query: string): boolean {
  return query.trim().length >= MIN_TRANSCRIPT_QUERY;
}

/** Transcript rows for search results (failed records are never in them). */
export function transcriptRows(records: HistoryRecord[], query: string): PaletteRow[] {
  return records.slice(0, TRANSCRIPT_LIMIT).map((r) => ({
    id: `record:${r.id}`,
    group: "Transcripts",
    label: snippet(r.text, 90),
    keywords: [],
    hint: r.appName ?? undefined,
    action: { type: "record", id: r.id, query: query.trim() },
  }));
}

/** Move the highlighted row with ↑/↓, wrapping at the ends. */
export function moveSelection(index: number, delta: number, count: number): number {
  if (count <= 0) return -1;
  if (index < 0) return delta > 0 ? 0 : count - 1;
  return (((index + delta) % count) + count) % count;
}

/** Rows grouped for display, keeping the order they arrive in. */
export function groupRows(rows: PaletteRow[]): { group: PaletteGroup; rows: PaletteRow[] }[] {
  const out: { group: PaletteGroup; rows: PaletteRow[] }[] = [];
  for (const row of rows) {
    const g = out.find((x) => x.group === row.group);
    if (g) g.rows.push(row);
    else out.push({ group: row.group, rows: [row] });
  }
  return out;
}

/** Whether a keydown is the palette shortcut (⌘K on macOS, Ctrl+K elsewhere). */
export function isPaletteShortcut(
  e: Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">,
  mac: boolean,
): boolean {
  return e.key.toLowerCase() === "k" && (mac ? e.metaKey : e.ctrlKey) && !e.altKey && !e.shiftKey;
}

/** Whether a keydown is the settings shortcut (⌘, / Ctrl+,). */
export function isSettingsShortcut(
  e: Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "altKey">,
  mac: boolean,
): boolean {
  return e.key === "," && (mac ? e.metaKey : e.ctrlKey) && !e.altKey;
}
