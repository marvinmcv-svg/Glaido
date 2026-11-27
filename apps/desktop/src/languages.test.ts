import { describe, expect, it } from "vitest";
import { defaultLanguage, LANGUAGE_CODES, languages, matchesLanguage } from "./languages";
import { chordKeys } from "./settings";

describe("languages", () => {
  it("offers close to 100 unique languages", () => {
    expect(LANGUAGE_CODES.length).toBeGreaterThanOrEqual(95);
    expect(new Set(LANGUAGE_CODES).size).toBe(LANGUAGE_CODES.length);
  });

  it("names languages in the UI locale and in themselves", () => {
    const de = languages("en").find((l) => l.code === "de")!;
    expect(de.name).toBe("German");
    expect(de.native).toBe("Deutsch");
  });

  it("never shows a bare code as a name", () => {
    for (const l of languages("en")) expect(l.name, l.code).not.toBe(l.code);
  });

  it("searches names, autonyms and codes, ignoring accents", () => {
    const all = languages("en");
    const find = (q: string) => all.filter((l) => matchesLanguage(l, q)).map((l) => l.code);
    expect(find("germ")).toContain("de");
    expect(find("deutsch")).toContain("de");
    expect(find("francais")).toContain("fr");
    expect(find("pt")).toContain("pt");
    expect(find("")).toHaveLength(all.length);
  });

  it("defaults from the system locale", () => {
    expect(defaultLanguage("de-AT")).toBe("de");
    expect(defaultLanguage("pt_BR")).toBe("pt");
    expect(defaultLanguage("xx-YY")).toBe("en");
  });

  it("splits chord settings into keys", () => {
    expect(chordKeys("left_ctrl+shift+space")).toEqual(["left_ctrl", "shift", "space"]);
  });
});
