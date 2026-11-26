# Spec 4.5: Personalization

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.5 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

#### 4.5.1 Dictionary
- **Two entry types.** *Vocabulary* (a word the recognizer should know: names, jargon, brands). *Correction* (misspelling → correct spelling, e.g. "Postgres" → "PostgreSQL").
- **50 characters max** per word and per misspelling, with a live counter. Duplicates are rejected ("Duplicate entry").
- **Dynamic tags** in the correction field: `{clipboard}`, `{datetime}`, `{timezone}`, inserted with a tag picker. Example: "paste that here" → clipboard contents. An entry containing a tag is a correction only and is never sent as a recognizer hint.
- Search covers **both sides** of each entry. Click to edit. Hover-delete asks for confirmation; **Shift-click deletes immediately**. **Delete all words.**
- **Auto-suggestions:** detect words the user fixes by hand after a dictation. After **2 fixes** of the same word (at least **4 chars**), show a notification: "You often correct X. Add to dictionary?" with **Add**. Observations expire after **30 days**. (The reference does this on macOS only. **★ BETTER:** support all OSes.)
- Add from History's "report" flow ("Add to my dictionary") and from the CLI and MCP.
- **Synced via the account** to every device, mobile included.

#### 4.5.2 Snippets
- **Trigger phrase → content.** The content can be multi-line and can include the dynamic tags.
- **Fires only when the *entire* dictation equals the trigger.** Matching ignores case and punctuation at either end. If two snippets share a trigger, the first wins. Snippets are **not** applied to commands.
- Search covers trigger and content. Click to edit; hover-delete asks for confirmation.
- Guidance: short, distinct triggers that you'd never dictate on their own.
- Reused by the **Text Snippet Expansion** extension (typed triggers) and listed in Spotlight. Synced via the account.

#### 4.5.3 Formatting (per-app style)
- **Three sections:**
  1. **All apps** (the default; no switch; can't be deleted).
  2. **Email** (built in; seeded with **mail.google.com** and **outlook.live.com** and no desktop apps; can be switched off but not deleted).
  3. **Custom rules.**
- **Styles:**
  - **Standard:** full capitals and punctuation.
  - **Casual:** capitals stay, fewer full stops.
  - **Lowercase:** no capitals, minimal punctuation.
- **Raw text** toggle: exactly what was said, skipping LLM cleanup. The style still applies. When Raw text is on, the custom prompt is hidden and dictionary accuracy drops.
- **Custom prompt:** max **500 chars** with a counter; Save is disabled when over.
- **Prompt inheritance:** an app with a rule gets the All apps prompt *first*, then its own on top. An app without a rule uses All apps.
- **Custom rule:** name, **Apps** (picker with *Most used*, up to 30 apps from the last 90 days of dictations that are still installed, then *All applications*; browsers and our own app are excluded), **Websites** (domains such as `github.com`; detected via the browser's active-tab URL; a website entry shows its address and saves without pressing Enter), style, Raw text, and custom prompt.
- **One app or website belongs to exactly one rule.** Apps already used elsewhere appear grayed out, with a tooltip naming the owning rule. Each rule has an on/off switch, so a rule can be tested by toggling it.
- **Migration notice:** when older flat rules are merged into All apps (numbered, truncated to 500), show a dismissible banner, remembered per device.
- **★ Preset rule gallery** (the reference publishes these as recipes on use-case pages; we ship them as one-click presets in our own wording):
  - **Writing code** (Cursor, VS Code): file names, functions and flags in code form (`getUserById`, `auth.ts`, `--force`); keep the user's wording and order; spoken steps become a numbered list.
  - **AI chat** (ChatGPT app, chatgpt.com, claude.ai, gemini.google.com): keep the wording; keep questions as questions and never answer them; spoken lists become numbered points; quoted material goes on its own lines.
  - **Team chat** (Slack, app.slack.com), Casual: short, keep the user's tone, no greeting or sign-off unless spoken, multi-point updates one per line.
  - **Email**: greeting on its own line, short paragraphs, a sign-off only if spoken.
  - **Terminal** (Terminal, iTerm2, Windows Terminal, Claude Code desktop): Standard with **Raw text on** so paths, flags and branch names are kept exactly.
  - **Friendly filter** (AI chat apps): strip toxic, angry and abusive language while keeping the request.

#### 4.5.4 Importers ★
The reference offers a **Wispr Flow import** (dictionary and snippets) during onboarding. We ship importers for **Wispr Flow, Superwhisper, Raycast/Alfred snippets, Espanso YAML and TextExpander CSV**, using each tool's own user-exportable files only.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
