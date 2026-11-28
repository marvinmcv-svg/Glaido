import { describe, expect, it } from "vitest";
import {
  doneText,
  fileBytes,
  FORMATS,
  type ImportReport,
  previewText,
  reasonText,
  suggestionText,
} from "./importer";

const report = (over: Partial<ImportReport> = {}): ImportReport => ({
  format: "wisprFlow",
  entries: [],
  snippets: [],
  skipped: [],
  converted: [],
  ...over,
});

describe("importer copy", () => {
  it("summarises a preview", () => {
    expect(previewText(report())).toBe("There's nothing new to add from this file.");
    expect(
      previewText(
        report({
          entries: [{ word: "Vocalis", misspelling: null }],
          snippets: [
            { trigger: "a", content: "b" },
            { trigger: "c", content: "d" },
          ],
          skipped: [{ item: "x", reason: { kind: "duplicate" } }],
        }),
      ),
    ).toBe("Ready to add 1 word and 2 snippets. 1 item will be left out.");
  });

  it("summarises a finished import", () => {
    expect(doneText(report())).toBe("Nothing was added.");
    expect(doneText(report({ snippets: [{ trigger: "a", content: "b" }] }))).toBe(
      "Added 1 snippet.",
    );
  });

  it("explains every skip reason", () => {
    expect(reasonText({ kind: "tooLong" })).toBe("Too long for Vocalis");
    expect(reasonText({ kind: "duplicate" })).toBe("Already in Vocalis");
    expect(reasonText({ kind: "invalid", detail: "no trigger" })).toContain("no trigger");
    expect(reasonText({ kind: "unsupported", feature: "forms" })).toContain("forms");
  });

  it("words a suggestion", () => {
    expect(suggestionText({ from: "kubernets", to: "Kubernetes", count: 2, lastSeen: 0 })).toBe(
      "Add “Kubernetes” to your dictionary? You fixed “kubernets” 2 times lately.",
    );
  });

  it("lists every backend format once", () => {
    const ids = FORMATS.map((f) => f.id);
    expect(new Set(ids).size).toBe(6);
  });

  it("reads file bytes", async () => {
    expect(await fileBytes(new Blob([new Uint8Array([1, 2, 255])]))).toEqual([1, 2, 255]);
  });
});
