/**
 * Formatting page types and pure helpers (spec 4.5.3). Types mirror
 * `vocalis_store::FormattingRule` and friends; the checks mirror
 * `vocalis_core::formatting` so the page can explain a problem before the
 * store rejects it. The store stays the authority.
 */

export type Style = "standard" | "casual" | "lowercase";
export type RuleKind = "all_apps" | "email" | "custom";
export type AppRef = { id: string; name: string };

export type FormattingRule = {
  id: string;
  kind: RuleKind;
  name: string;
  enabled: boolean;
  style: Style;
  raw: boolean;
  prompt: string;
  apps: AppRef[];
  sites: string[];
  position: number;
  createdAt: number;
  updatedAt: number;
};

export type RuleDraft = Pick<
  FormattingRule,
  "name" | "enabled" | "style" | "raw" | "prompt" | "apps" | "sites"
>;

export type UsedApp = AppRef & { count: number };
export type PresetApp = AppRef & { os: "macos" | "windows" | "linux" };
export type Preset = {
  id: string;
  name: string;
  summary: string;
  style: Style;
  raw: boolean;
  prompt: string;
  apps: PresetApp[];
  sites: string[];
  builtinEmail: boolean;
};
export type Skipped = { item: string; owner: string };
export type PresetOutcome = {
  rule: FormattingRule | null;
  skipped: Skipped[];
  keptPrompt: boolean;
};

export const MAX_PROMPT_CHARS = 500;
export const MAX_NAME_CHARS = 80;

export const STYLES: { id: Style; label: string; description: string; example: string }[] = [
  {
    id: "standard",
    label: "Standard",
    description: "Capital letters and full punctuation.",
    example: "Sounds good. I'll send the notes after lunch.",
  },
  {
    id: "casual",
    label: "Casual",
    description: "Capitals stay, fewer full stops.",
    example: "Sounds good, I'll send the notes after lunch",
  },
  {
    id: "lowercase",
    label: "Lowercase",
    description: "No capitals, only the punctuation that matters.",
    example: "sounds good, i'll send the notes after lunch",
  },
];

/** Characters as the store counts them (Unicode scalar values). */
export function charCount(s: string): number {
  return [...s].length;
}

export function promptTooLong(prompt: string): boolean {
  return charCount(prompt.trim()) > MAX_PROMPT_CHARS;
}

/** A site entry or URL as a bare host, like `normalize_host` in core. */
export function normalizeSite(s: string): string {
  let h = s.trim().toLowerCase();
  const scheme = h.indexOf("://");
  if (scheme >= 0) h = h.slice(scheme + 3);
  h = h.split(/[/?#]/)[0] ?? "";
  const at = h.lastIndexOf("@");
  if (at >= 0) h = h.slice(at + 1);
  h = h.split(":")[0] ?? "";
  if (h.startsWith("www.")) h = h.slice(4);
  return h.replace(/\.+$/, "");
}

/** Whether a normalized host is acceptable to the store. */
export function isValidSite(host: string): boolean {
  return (
    host.length > 0 &&
    !host.startsWith(".") &&
    !host.startsWith("-") &&
    !host.includes("..") &&
    /^[\p{L}\p{N}.-]+$/u.test(host)
  );
}

export function draftOf(rule: FormattingRule): RuleDraft {
  const { name, enabled, style, raw, prompt, apps, sites } = rule;
  return { name, enabled, style, raw, prompt, apps: [...apps], sites: [...sites] };
}

export function emptyDraft(): RuleDraft {
  return {
    name: "",
    enabled: true,
    style: "standard",
    raw: false,
    prompt: "",
    apps: [],
    sites: [],
  };
}

/** The rule that owns this app, other than `exceptId`, if any. */
export function appOwner(
  rules: FormattingRule[],
  appId: string,
  exceptId: string | null,
): string | null {
  const id = appId.trim().toLowerCase();
  const r = rules.find(
    (r) => r.id !== exceptId && r.apps.some((a) => a.id.trim().toLowerCase() === id),
  );
  return r ? r.name : null;
}

export function siteOwner(
  rules: FormattingRule[],
  site: string,
  exceptId: string | null,
): string | null {
  const host = normalizeSite(site);
  const r = rules.find((r) => r.id !== exceptId && r.sites.some((s) => normalizeSite(s) === host));
  return r ? r.name : null;
}

/** Add a typed website to the draft. Returns an error message, or null. */
export function addSite(
  draft: RuleDraft,
  typed: string,
  rules: FormattingRule[],
  exceptId: string | null,
): { draft: RuleDraft; error: string | null } {
  if (!typed.trim()) return { draft, error: null };
  const host = normalizeSite(typed);
  if (!isValidSite(host))
    return { draft, error: `"${typed.trim()}" doesn't look like a website address.` };
  const owner = siteOwner(rules, host, exceptId);
  if (owner) return { draft, error: `${host} already belongs to "${owner}".` };
  if (draft.sites.includes(host)) return { draft, error: null };
  return { draft: { ...draft, sites: [...draft.sites, host] }, error: null };
}

export function toggleApp(draft: RuleDraft, app: AppRef): RuleDraft {
  const has = draft.apps.some((a) => a.id.toLowerCase() === app.id.toLowerCase());
  return {
    ...draft,
    apps: has
      ? draft.apps.filter((a) => a.id.toLowerCase() !== app.id.toLowerCase())
      : [...draft.apps, { id: app.id, name: app.name }],
  };
}

/** Why Save is disabled, or null when the draft can be saved. */
export function saveBlocker(draft: RuleDraft, kind: RuleKind): string | null {
  if (kind === "custom" && !draft.name.trim()) return "Give the rule a name.";
  if (kind === "custom" && charCount(draft.name.trim()) > MAX_NAME_CHARS)
    return `Names can be at most ${MAX_NAME_CHARS} characters.`;
  if (!draft.raw && promptTooLong(draft.prompt))
    return `The prompt is over ${MAX_PROMPT_CHARS} characters.`;
  return null;
}

/** "Slack, Code and github.com" for a rule's summary line. */
export function targetsLabel(rule: Pick<FormattingRule, "kind" | "apps" | "sites">): string {
  if (rule.kind === "all_apps") return "Every app without a rule of its own";
  const items = [...rule.apps.map((a) => a.name || a.id), ...rule.sites];
  if (items.length === 0) return "No apps or websites yet";
  if (items.length <= 3) return joinAnd(items);
  return `${items.slice(0, 2).join(", ")} and ${items.length - 2} more`;
}

function joinAnd(items: string[]): string {
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items.at(-1)}`;
}

/** What to tell the user after a preset was added (or not). */
export function presetMessage(preset: Pick<Preset, "name">, out: PresetOutcome): string {
  const skipped = out.skipped.map((s) => `${s.item} (in "${s.owner}")`);
  if (!out.rule) {
    return `Nothing added: everything in ${preset.name} already belongs to another rule: ${joinAnd(skipped)}.`;
  }
  let msg =
    out.rule.kind === "email"
      ? `Updated the Email rule with the ${preset.name} preset.`
      : `Added "${out.rule.name}".`;
  if (out.keptPrompt) msg += " Your own Email prompt was kept.";
  if (skipped.length) msg += ` Skipped ${joinAnd(skipped)}.`;
  return msg;
}

export type PickerItem = AppRef & { owner: string | null; selected: boolean };

/** The app picker: Most used first, then All applications (without repeats). */
export function pickerSections(
  mostUsed: AppRef[],
  installed: AppRef[],
  query: string,
  rules: FormattingRule[],
  draft: RuleDraft,
  exceptId: string | null,
): { mostUsed: PickerItem[]; all: PickerItem[] } {
  const q = query.trim().toLowerCase();
  const match = (a: AppRef) =>
    !q || a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q);
  const item = (a: AppRef): PickerItem => ({
    id: a.id,
    name: a.name,
    owner: appOwner(rules, a.id, exceptId),
    selected: draft.apps.some((d) => d.id.toLowerCase() === a.id.toLowerCase()),
  });
  const used = new Set(mostUsed.map((a) => a.id.toLowerCase()));
  return {
    mostUsed: mostUsed.filter(match).map(item),
    all: installed.filter((a) => !used.has(a.id.toLowerCase()) && match(a)).map(item),
  };
}
