import { describe, expect, it } from "vitest";
import type { HistoryRecord } from "./history";
import {
  buildRows,
  canSearchTranscripts,
  filterRows,
  groupRows,
  isPaletteShortcut,
  isSettingsShortcut,
  moveSelection,
  TRANSCRIPT_LIMIT,
  transcriptRows,
} from "./paletteItems";

const PAGES = ["Home", "Dictionary", "Snippets", "Formatting", "Insights", "Settings"];
const rows = buildRows({ pages: PAGES, settingsShortcut: "Ctrl+,", theme: "dark" });
const labels = (q: string) => filterRows(rows, q).map((r) => r.label);

describe("buildRows", () => {
  it("has navigation, quick-add, settings and theme rows", () => {
    expect(rows.filter((r) => r.group === "Go to").map((r) => r.label)).toEqual([
      "Home",
      "Formatting",
      "Dictionary",
      "Snippets",
      "Insights",
    ]);
    expect(rows.filter((r) => r.group === "Add").map((r) => r.label)).toEqual([
      "Add words",
      "Add snippet",
      "Add formatting rule",
    ]);
    expect(rows.find((r) => r.label === "General")?.hint).toBe("Ctrl+,");
    expect(rows.at(-1)?.label).toBe("Switch to light theme");
    expect(buildRows({ pages: PAGES, settingsShortcut: "", theme: "light" }).at(-1)?.label).toBe(
      "Switch to dark theme",
    );
  });

  it("omits rows for pages this build doesn't have", () => {
    const few = buildRows({ pages: ["Home", "Settings"], settingsShortcut: "", theme: "dark" });
    expect(few.some((r) => r.label === "Snippets" || r.label === "Add snippet")).toBe(false);
    expect(few.some((r) => r.label === "Home")).toBe(true);
  });

  it("quick-add rows go to the page's add form", () => {
    expect(rows.find((r) => r.label === "Add snippet")?.action).toEqual({
      type: "page",
      page: "Snippets",
      anchor: "add",
    });
  });
});

describe("filterRows", () => {
  it("returns everything for an empty query", () => {
    expect(filterRows(rows, "  ")).toHaveLength(rows.length);
  });

  it("ranks label prefixes first", () => {
    expect(labels("sn")[0]).toBe("Snippets");
    expect(labels("add")).toEqual(["Add words", "Add snippet", "Add formatting rule"]);
  });

  it("finds pages by synonym", () => {
    expect(labels("personalization")).toContain("Formatting");
    expect(labels("stats")).toContain("Insights");
    expect(labels("dark mode")).toEqual(["Switch to light theme"]);
  });

  it("ignores case and accents", () => {
    expect(labels("INSIGHTS")).toEqual(["Insights"]);
    expect(labels("snïppets")).toEqual(["Snippets"]);
  });

  it("synonyms for absent pages match nothing", () => {
    expect(labels("mcp")).toEqual([]);
  });
});

describe("transcripts", () => {
  const rec = (i: number): HistoryRecord => ({
    id: `r${i}`,
    kind: "dictation",
    status: "ok",
    text: `note number ${i}`,
    appName: i === 0 ? "Mail" : null,
    appId: null,
    url: null,
    fieldKind: null,
    words: 3,
    durationMs: 1000,
    wpm: 180,
    audioPath: null,
    errorCode: null,
    createdAt: i,
  });

  it("needs two characters", () => {
    expect(canSearchTranscripts("a")).toBe(false);
    expect(canSearchTranscripts(" ab ")).toBe(true);
  });

  it("caps results at 15 and opens the record in Activity", () => {
    const r = transcriptRows(
      Array.from({ length: 40 }, (_, i) => rec(i)),
      " note ",
    );
    expect(r).toHaveLength(TRANSCRIPT_LIMIT);
    expect(r[0]).toMatchObject({
      group: "Transcripts",
      label: "note number 0",
      hint: "Mail",
      action: { type: "record", id: "r0", query: "note" },
    });
  });
});

describe("keyboard", () => {
  it("moves and wraps", () => {
    expect(moveSelection(-1, 1, 3)).toBe(0);
    expect(moveSelection(-1, -1, 3)).toBe(2);
    expect(moveSelection(2, 1, 3)).toBe(0);
    expect(moveSelection(0, -1, 3)).toBe(2);
    expect(moveSelection(0, 1, 0)).toBe(-1);
  });

  it("recognizes the shortcuts per platform", () => {
    const k = { key: "k", metaKey: false, ctrlKey: true, altKey: false, shiftKey: false };
    expect(isPaletteShortcut(k, false)).toBe(true);
    expect(isPaletteShortcut(k, true)).toBe(false);
    expect(isPaletteShortcut({ ...k, ctrlKey: false, metaKey: true }, true)).toBe(true);
    expect(isPaletteShortcut({ ...k, shiftKey: true }, false)).toBe(false);
    expect(
      isSettingsShortcut({ key: ",", metaKey: true, ctrlKey: false, altKey: false }, true),
    ).toBe(true);
  });
});

describe("groupRows", () => {
  it("keeps arrival order", () => {
    const g = groupRows(filterRows(rows, "add"));
    expect(g.map((x) => x.group)).toEqual(["Add"]);
  });
});
