import { describe, expect, it } from "vitest";
import {
  charCount,
  type DictionaryEntry,
  draftFor,
  draftMisspelling,
  DUPLICATE_ENTRY,
  entryError,
  errorText,
  filterEntries,
  filterSnippets,
  insertAt,
  normalizePhrase,
  preview,
  previewBackend,
  shadowedIds,
  sharedTrigger,
  snippetError,
  type SnippetEntry,
} from "./personal";

const word = (id: string, w: string, m: string | null = null, at = 0): DictionaryEntry => ({
  id,
  word: w,
  misspelling: m,
  createdAt: at,
  updatedAt: at,
});
const snip = (id: string, trigger: string, content: string, at = 0): SnippetEntry => ({
  id,
  trigger,
  content,
  createdAt: at,
  updatedAt: at,
});

describe("dictionary drafts", () => {
  const entries = [word("a", "Vocalis"), word("b", "PostgreSQL", "postgres")];

  it("counts characters like the backend", () => {
    expect(charCount("  héllo ")).toBe(5);
    expect(charCount("😀".repeat(50))).toBe(50);
  });

  it("rejects duplicates on both sides, ignoring case and spaces", () => {
    const vocab = { kind: "vocabulary" as const, word: " vocalis ", misspelling: "" };
    expect(entryError(vocab, entries, null)).toBe(DUPLICATE_ENTRY);
    expect(entryError(vocab, entries, "a")).toBeNull();
    const corr = { kind: "correction" as const, word: "postgresql", misspelling: "POSTGRES" };
    expect(entryError(corr, entries, null)).toBe(DUPLICATE_ENTRY);
    // Same word, different misspelling: a different entry.
    expect(entryError({ ...corr, misspelling: "postgre" }, entries, null)).toBeNull();
  });

  it("treats a blank misspelling as vocabulary", () => {
    const d = { kind: "correction" as const, word: "Vocalis", misspelling: "  " };
    expect(draftMisspelling(d)).toBeNull();
    expect(entryError(d, entries, null)).toBe(DUPLICATE_ENTRY);
    expect(draftMisspelling({ ...d, kind: "vocabulary", misspelling: "x" })).toBeNull();
  });

  it("enforces 50 characters per side", () => {
    const ok = { kind: "correction" as const, word: "x".repeat(50), misspelling: "y".repeat(50) };
    expect(entryError(ok, [], null)).toBeNull();
    expect(entryError({ ...ok, word: "x".repeat(51) }, [], null)).toMatch(/50/);
    expect(entryError({ ...ok, misspelling: "y".repeat(51) }, [], null)).toMatch(/50/);
  });

  it("requires a correction for tags and a word", () => {
    expect(
      entryError({ kind: "vocabulary", word: "{clipboard}", misspelling: "" }, [], null),
    ).toMatch(/tag/i);
    expect(
      entryError({ kind: "correction", word: "{clipboard}", misspelling: "paste it" }, [], null),
    ).toBeNull();
    expect(
      entryError({ kind: "vocabulary", word: "  ", misspelling: "" }, [], null),
    ).not.toBeNull();
  });

  it("opens an entry in the right mode", () => {
    expect(draftFor(entries[1]!)).toEqual({
      kind: "correction",
      word: "PostgreSQL",
      misspelling: "postgres",
    });
    expect(draftFor(null).kind).toBe("vocabulary");
  });

  it("searches both sides", () => {
    expect(filterEntries(entries, "GRES").map((e) => e.id)).toEqual(["b"]);
    expect(filterEntries(entries, "voc").map((e) => e.id)).toEqual(["a"]);
    expect(filterEntries(entries, "  ")).toHaveLength(2);
  });
});

describe("tags", () => {
  it("inserts at the caret or over the selection", () => {
    expect(insertAt("Link: ", "{clipboard}", 6, 6)).toEqual({
      value: "Link: {clipboard}",
      caret: 17,
    });
    expect(insertAt("a XX b", "{datetime}", 2, 4)).toEqual({ value: "a {datetime} b", caret: 12 });
    expect(insertAt("abc", "{timezone}", null, null).value).toBe("abc{timezone}");
    expect(insertAt("abc", "!", 99, 99)).toEqual({ value: "abc!", caret: 4 });
  });
});

describe("snippets", () => {
  const list = [
    snip("1", "my address", "1 Main St\nSpringfield", 1),
    snip("2", "My address!", "two", 2),
  ];

  it("normalizes triggers like the backend", () => {
    expect(normalizePhrase("  ¿My Address?! ")).toBe("my address");
    expect(normalizePhrase("...")).toBe("");
    expect(normalizePhrase("e-mail sig")).toBe("e-mail sig");
  });

  it("finds a shared trigger and which snippets never fire", () => {
    expect(sharedTrigger("MY ADDRESS.", list, null)?.id).toBe("1");
    expect(sharedTrigger("my address", list, "1")?.id).toBe("2");
    expect(sharedTrigger("other", list, null)).toBeNull();
    expect(sharedTrigger("  ", list, null)).toBeNull();
    expect([...shadowedIds(list)]).toEqual(["2"]);
  });

  it("validates trigger and content", () => {
    expect(snippetError({ trigger: "sig", content: "Best,\nSam" })).toBeNull();
    expect(snippetError({ trigger: "?!", content: "x" })).toMatch(/trigger/);
    expect(snippetError({ trigger: "t".repeat(201), content: "x" })).toMatch(/200/);
    expect(snippetError({ trigger: "sig", content: " \n " })).toMatch(/text/);
  });

  it("searches trigger and content", () => {
    expect(filterSnippets(list, "springfield").map((s) => s.id)).toEqual(["1"]);
    expect(filterSnippets(list, "ADDRESS")).toHaveLength(2);
  });

  it("previews multi-line content on one line", () => {
    expect(preview("1 Main St\n  Springfield\n")).toBe("1 Main St Springfield");
    expect(preview("x".repeat(10), 5)).toBe("xxxx…");
  });
});

describe("preview backend", () => {
  it("adds, edits, rejects duplicates and deletes", async () => {
    const call = previewBackend();
    const a = await call<DictionaryEntry>("dictionary_add", {
      word: " Vocalis ",
      misspelling: null,
    });
    expect(a.word).toBe("Vocalis");
    await expect(call("dictionary_add", { word: "vocalis" })).rejects.toThrow(DUPLICATE_ENTRY);
    await call("dictionary_add", { word: "PostgreSQL", misspelling: "postgres" });
    const listed = await call<DictionaryEntry[]>("dictionary_list");
    expect(listed.map((e) => e.word)).toEqual(["PostgreSQL", "Vocalis"]);
    await call("dictionary_update", { id: a.id, word: "Vocalis app", misspelling: null });
    expect((await call<DictionaryEntry[]>("dictionary_list")).map((e) => e.word)).toContain(
      "Vocalis app",
    );
    expect(await call<number>("dictionary_delete_all")).toBe(2);

    const s = await call<SnippetEntry>("snippets_add", { trigger: "sig", content: "Best" });
    await expect(call("snippets_add", { trigger: "", content: "x" })).rejects.toThrow();
    await call("snippets_delete", { id: s.id });
    expect(await call<SnippetEntry[]>("snippets_list")).toEqual([]);
  });

  it("reports errors as sentences", () => {
    expect(errorText("Duplicate entry")).toBe("Duplicate entry");
    expect(errorText(new Error("x"))).toBe("x");
    expect(errorText(42)).toBe("Something went wrong.");
  });
});
