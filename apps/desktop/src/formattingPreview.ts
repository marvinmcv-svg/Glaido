/**
 * An in-memory stand-in for the formatting commands, so the page works in a
 * plain browser (`pnpm dev`) without Tauri. Sample data only; the real rules,
 * presets and app lists come from Rust.
 */
import {
  appOwner,
  type AppRef,
  type FormattingRule,
  type Preset,
  type PresetOutcome,
  type RuleDraft,
  siteOwner,
  type Skipped,
  type UsedApp,
} from "./formatting";

const SAMPLE_PRESETS: Preset[] = [
  {
    id: "writing-code",
    name: "Writing code",
    summary: "Identifiers, files and flags in code form; spoken steps become a numbered list.",
    style: "standard",
    raw: false,
    prompt: "Write file names, function and variable names, commands and flags in code form.",
    apps: [{ id: "com.microsoft.VSCode", name: "Visual Studio Code", os: "macos" }],
    sites: [],
    builtinEmail: false,
  },
  {
    id: "ai-chat",
    name: "AI chat",
    summary: "Prompts for assistants: questions stay questions, lists become numbered points.",
    style: "standard",
    raw: false,
    prompt: "This text is a prompt I am writing for an AI assistant. Keep my wording.",
    apps: [{ id: "com.openai.chat", name: "ChatGPT", os: "macos" }],
    sites: ["chatgpt.com", "claude.ai", "gemini.google.com"],
    builtinEmail: false,
  },
  {
    id: "team-chat",
    name: "Team chat",
    summary: "Short messages in your own tone, one point per line, no added greetings.",
    style: "casual",
    raw: false,
    prompt: "Keep it short and in my own tone.",
    apps: [{ id: "com.tinyspeck.slackmacgap", name: "Slack", os: "macos" }],
    sites: ["app.slack.com"],
    builtinEmail: false,
  },
  {
    id: "email",
    name: "Email",
    summary: "Greeting on its own line, short paragraphs, a sign-off only when you say one.",
    style: "standard",
    raw: false,
    prompt: "Put the greeting on its own line, then write short paragraphs.",
    apps: [{ id: "com.apple.mail", name: "Mail", os: "macos" }],
    sites: [],
    builtinEmail: true,
  },
  {
    id: "terminal",
    name: "Terminal",
    summary: "Raw text, so paths, flags and branch names come out exactly as spoken.",
    style: "standard",
    raw: true,
    prompt: "",
    apps: [{ id: "com.apple.Terminal", name: "Terminal", os: "macos" }],
    sites: [],
    builtinEmail: false,
  },
  {
    id: "friendly-filter",
    name: "Friendly filter",
    summary: "Takes the heat out of what you send to AI chats while keeping the request.",
    style: "standard",
    raw: false,
    prompt: "Leave out insults, swearing and angry or abusive wording.",
    apps: [{ id: "com.openai.chat", name: "ChatGPT", os: "macos" }],
    sites: ["chatgpt.com", "claude.ai", "gemini.google.com"],
    builtinEmail: false,
  },
];

const SAMPLE_USED: UsedApp[] = [
  { id: "com.tinyspeck.slackmacgap", name: "Slack", count: 42 },
  { id: "com.microsoft.VSCode", name: "Visual Studio Code", count: 31 },
  { id: "com.apple.Notes", name: "Notes", count: 9 },
];

const SAMPLE_INSTALLED: AppRef[] = [
  { id: "com.apple.mail", name: "Mail" },
  { id: "com.apple.Notes", name: "Notes" },
  { id: "notion.id", name: "Notion" },
  { id: "com.tinyspeck.slackmacgap", name: "Slack" },
  { id: "com.apple.Terminal", name: "Terminal" },
  { id: "com.microsoft.VSCode", name: "Visual Studio Code" },
];

type Args = Record<string, unknown>;

export function createPreviewBackend(now: () => number = Date.now) {
  let n = 0;
  const make = (kind: FormattingRule["kind"], d: RuleDraft): FormattingRule => ({
    id: `preview-${++n}`,
    kind,
    ...d,
    position: n,
    createdAt: now(),
    updatedAt: now(),
  });
  let rules: FormattingRule[] = [
    make("all_apps", {
      name: "All apps",
      enabled: true,
      style: "standard",
      raw: false,
      prompt: "",
      apps: [],
      sites: [],
    }),
    make("email", {
      name: "Email",
      enabled: true,
      style: "standard",
      raw: false,
      prompt: "",
      apps: [],
      sites: ["mail.google.com", "outlook.live.com"],
    }),
  ];

  const check = (id: string | null, d: RuleDraft) => {
    for (const a of d.apps) {
      const o = appOwner(rules, a.id, id);
      if (o) throw new Error(`${a.name} is already used by "${o}"`);
    }
    for (const s of d.sites) {
      const o = siteOwner(rules, s, id);
      if (o) throw new Error(`${s} is already used by "${o}"`);
    }
    if ([...d.prompt.trim()].length > 500)
      throw new Error("The prompt is longer than 500 characters.");
  };

  const save = (id: string | null, d: RuleDraft): FormattingRule => {
    check(id, d);
    if (id === null) {
      if (!d.name.trim()) throw new Error("Give the rule a name.");
      const r = make("custom", { ...d, name: d.name.trim(), prompt: d.prompt.trim() });
      rules = [...rules, r];
      return r;
    }
    const old = rules.find((r) => r.id === id);
    if (!old) throw new Error("record not found");
    if (old.kind === "all_apps" && !d.enabled)
      throw new Error("the All apps rule cannot be switched off");
    const r: FormattingRule = {
      ...old,
      ...d,
      name: old.kind === "custom" ? d.name.trim() : old.name,
      prompt: d.prompt.trim(),
      updatedAt: now(),
    };
    rules = rules.map((x) => (x.id === id ? r : x));
    return r;
  };

  const addPreset = (presetId: string): PresetOutcome => {
    const p = SAMPLE_PRESETS.find((x) => x.id === presetId);
    if (!p) throw new Error(`There is no preset called "${presetId}".`);
    const target = p.builtinEmail ? rules.find((r) => r.kind === "email") : undefined;
    const skipped: Skipped[] = [];
    const apps: AppRef[] = [...(target?.apps ?? [])];
    const sites: string[] = [...(target?.sites ?? [])];
    for (const a of p.apps) {
      const o = appOwner(rules, a.id, target?.id ?? null);
      if (o) skipped.push({ item: a.name, owner: o });
      else if (!apps.some((x) => x.id === a.id)) apps.push({ id: a.id, name: a.name });
    }
    for (const s of p.sites) {
      const o = siteOwner(rules, s, target?.id ?? null);
      if (o) skipped.push({ item: s, owner: o });
      else if (!sites.includes(s)) sites.push(s);
    }
    if (!target && apps.length === 0 && sites.length === 0)
      return { rule: null, skipped, keptPrompt: false };
    const keptPrompt = !!target && target.prompt.trim() !== "" && target.prompt !== p.prompt;
    const d: RuleDraft = {
      name: p.name,
      enabled: true,
      style: p.style,
      raw: p.raw,
      prompt: keptPrompt && target ? target.prompt : p.prompt,
      apps,
      sites,
    };
    return { rule: save(target?.id ?? null, d), skipped, keptPrompt };
  };

  return function handle(cmd: string, args: Args = {}): unknown {
    switch (cmd) {
      case "rules_get":
        return rules;
      case "rule_save":
        return save((args.id as string | null) ?? null, args.rule as RuleDraft);
      case "rule_toggle": {
        const r = rules.find((x) => x.id === args.id);
        if (!r) throw new Error("record not found");
        return save(r.id, { ...r, enabled: args.enabled as boolean });
      }
      case "rule_delete": {
        const r = rules.find((x) => x.id === args.id);
        if (r?.kind === "all_apps") throw new Error("The All apps rule can't be deleted.");
        if (r?.kind === "email")
          throw new Error("The Email rule can be switched off, but not deleted.");
        rules = rules.filter((x) => x.id !== args.id);
        return null;
      }
      case "rule_add_preset":
        return addPreset(args.presetId as string);
      case "formatting_presets":
        return SAMPLE_PRESETS;
      case "apps_most_used":
        return SAMPLE_USED;
      case "apps_installed":
        return SAMPLE_INSTALLED;
      default:
        throw new Error(`Unknown command ${cmd}`);
    }
  };
}
