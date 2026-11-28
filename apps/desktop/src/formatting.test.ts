import { describe, expect, it } from "vitest";
import {
  addSite,
  appOwner,
  charCount,
  emptyDraft,
  type FormattingRule,
  isValidSite,
  normalizeSite,
  pickerSections,
  presetMessage,
  promptTooLong,
  saveBlocker,
  targetsLabel,
  toggleApp,
} from "./formatting";
import { createPreviewBackend } from "./formattingPreview";

const rule = (over: Partial<FormattingRule>): FormattingRule => ({
  id: "r",
  kind: "custom",
  name: "Rule",
  enabled: true,
  style: "standard",
  raw: false,
  prompt: "",
  apps: [],
  sites: [],
  position: 0,
  createdAt: 0,
  updatedAt: 0,
  ...over,
});

const rules: FormattingRule[] = [
  rule({ id: "all", kind: "all_apps", name: "All apps" }),
  rule({ id: "email", kind: "email", name: "Email", sites: ["mail.google.com"] }),
  rule({ id: "chat", name: "Team chat", apps: [{ id: "Slack", name: "Slack" }] }),
];

describe("prompt limit", () => {
  it("counts characters, not UTF-16 units, and allows exactly 500", () => {
    expect(charCount("😀é")).toBe(2);
    expect(promptTooLong("😀".repeat(500))).toBe(false);
    expect(promptTooLong("x".repeat(501))).toBe(true);
    expect(promptTooLong(`  ${"x".repeat(500)}  `)).toBe(false);
  });

  it("blocks Save when over the limit, unless Raw text hides the prompt", () => {
    const d = { ...emptyDraft(), name: "A", prompt: "x".repeat(501) };
    expect(saveBlocker(d, "custom")).toMatch(/500/);
    expect(saveBlocker({ ...d, raw: true }, "custom")).toBeNull();
    expect(saveBlocker({ ...d, prompt: "", name: " " }, "custom")).toMatch(/name/);
    expect(saveBlocker({ ...d, prompt: "", name: "" }, "all_apps")).toBeNull();
  });
});

describe("sites", () => {
  it("normalizes like the core does", () => {
    expect(normalizeSite("HTTPS://user@www.GitHub.com:443/a?b#c")).toBe("github.com");
    expect(normalizeSite(" docs.rs/ ")).toBe("docs.rs");
    expect(isValidSite("github.com")).toBe(true);
    expect(isValidSite("not a site")).toBe(false);
    expect(isValidSite("a..b")).toBe(false);
  });

  it("adds a typed site, rejecting ones another rule owns", () => {
    const d = emptyDraft();
    const ok = addSite(d, "https://www.github.com/org", rules, null);
    expect(ok.error).toBeNull();
    expect(ok.draft.sites).toEqual(["github.com"]);
    expect(addSite(ok.draft, "github.com", rules, null).draft.sites).toEqual(["github.com"]);
    expect(addSite(d, "mail.google.com/mail", rules, null).error).toMatch(/"Email"/);
    expect(addSite(d, "mail.google.com", rules, "email").error).toBeNull();
    expect(addSite(d, "   ", rules, null)).toEqual({ draft: d, error: null });
    expect(addSite(d, "two words", rules, null).error).toMatch(/website/);
  });
});

describe("apps", () => {
  it("names the owner of an app, ignoring the rule being edited", () => {
    expect(appOwner(rules, "slack", null)).toBe("Team chat");
    expect(appOwner(rules, "slack", "chat")).toBeNull();
    expect(appOwner(rules, "Notes", null)).toBeNull();
  });

  it("toggles apps case-insensitively", () => {
    let d = toggleApp(emptyDraft(), { id: "Code", name: "Code" });
    expect(d.apps).toHaveLength(1);
    d = toggleApp(d, { id: "code", name: "Code" });
    expect(d.apps).toHaveLength(0);
  });

  it("lists Most used first, then the rest, greying owned apps", () => {
    const used = [{ id: "Slack", name: "Slack" }];
    const installed = [
      { id: "slack", name: "Slack" },
      { id: "Code", name: "Visual Studio Code" },
    ];
    const s = pickerSections(used, installed, "", rules, emptyDraft(), null);
    expect(s.mostUsed).toEqual([
      { id: "Slack", name: "Slack", owner: "Team chat", selected: false },
    ]);
    expect(s.all.map((a) => a.id)).toEqual(["Code"]);
    const q = pickerSections(used, installed, "visual", rules, emptyDraft(), null);
    expect(q.mostUsed).toHaveLength(0);
    expect(q.all).toHaveLength(1);
  });
});

describe("labels", () => {
  it("summarizes targets", () => {
    expect(targetsLabel(rules[0]!)).toMatch(/Every app/);
    expect(targetsLabel(rule({}))).toMatch(/No apps/);
    expect(
      targetsLabel(rule({ apps: [{ id: "a", name: "Slack" }], sites: ["app.slack.com"] })),
    ).toBe("Slack and app.slack.com");
    expect(targetsLabel(rule({ sites: ["a.com", "b.com", "c.com", "d.com"] }))).toBe(
      "a.com, b.com and 2 more",
    );
  });

  it("explains what a preset did", () => {
    const added = rule({ name: "AI chat" });
    expect(
      presetMessage({ name: "AI chat" }, { rule: added, skipped: [], keptPrompt: false }),
    ).toBe('Added "AI chat".');
    expect(
      presetMessage(
        { name: "Friendly filter" },
        { rule: null, skipped: [{ item: "claude.ai", owner: "AI chat" }], keptPrompt: false },
      ),
    ).toMatch(/^Nothing added.*claude\.ai \(in "AI chat"\)/);
    expect(
      presetMessage(
        { name: "Email" },
        { rule: rules[1]!, skipped: [{ item: "Mail", owner: "Mine" }], keptPrompt: true },
      ),
    ).toBe(
      'Updated the Email rule with the Email preset. Your own Email prompt was kept. Skipped Mail (in "Mine").',
    );
  });
});

describe("browser preview backend", () => {
  it("seeds the built-in rules and enforces the same rules as the store", () => {
    const call = createPreviewBackend(() => 1);
    const list = call("rules_get") as FormattingRule[];
    expect(list.map((r) => r.kind)).toEqual(["all_apps", "email"]);
    expect(() => call("rule_toggle", { id: list[0]!.id, enabled: false })).toThrow();
    expect(() => call("rule_delete", { id: list[1]!.id })).toThrow(/not deleted/);
    const ai = call("rule_add_preset", { presetId: "ai-chat" }) as { rule: FormattingRule };
    expect(ai.rule.sites).toContain("claude.ai");
    const ff = call("rule_add_preset", { presetId: "friendly-filter" }) as {
      rule: FormattingRule | null;
      skipped: unknown[];
    };
    expect(ff.rule).toBeNull();
    expect(ff.skipped).toHaveLength(4);
    expect(() =>
      call("rule_save", { id: null, rule: { ...emptyDraft(), name: "X", sites: ["claude.ai"] } }),
    ).toThrow(/AI chat/);
  });
});
