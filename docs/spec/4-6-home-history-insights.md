# Spec 4.6: Home, History and Insights

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.6 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Home dashboard**
- Stat cards: **Time saved** (e.g. "123h 46m", with a human comparison such as "about a full work week"), **Dictation speed** (WPM, with a "×N faster than typing" comparison), **Day streak** (with a friendly label).
- A **Go Pro** button at top right appears for free users, and on the Dictionary, Snippets, Formatting, Extensions and Insights pages too.
- An **Activity** panel (the history list).

**Activity / History**
- Every dictation **and** command, **grouped by day**, newest first. Each row shows a snippet, the app (with its icon) and the time.
- Command rows carry a **Command** chip, a **Running** chip while in progress, and a **tool count** chip when done.
- A shield icon and **privacy tooltip**: stored on this device, encrypted.
- **Search** in the panel header: at least 2 characters, matches anywhere in the transcript, up to **50 results**. Failed records aren't searchable; find them by date.
- **Dictation record:** full transcript, copy, **audio player**, **Download audio**, **Retry transcription** (on hover, next to copy).
- **Command record:** the request, its context (selection or image), the answer, the tools that ran, and a Stop button on slow tools.
- **Failed record:** "Transcription failed. Your audio is safe," with **Retry** in the row and in the record. The failure notification also has **Retry**, which re-transcribes into the record without pasting. Unlimited retries.
- **Report a record** (thumbs-down): "Something went wrong" with categories. For a wrong word, type the right spelling and tick **Add to my dictionary**. The report shares **only that one record** (text, never audio) with support.
- **Storage:** transcripts in an encrypted local DB. Audio as **`.wav.enc`** (AES-GCM). **Audio is kept for the last 100 recordings**; older records keep only the text.
- **Clear history** (Settings → Account) wipes everything, including clipboard search data. It is permanent and irreversible. The reference can't delete a single record. **★ BETTER:** support per-record delete and auto-expiry options.

**Insights page:** words dictated, active days, and command usage over time. Charts and trends.

**Command Palette (⌘K / Ctrl+K)**
- Navigation: Home, Formatting, Dictionary, Snippets, Extensions, Insights, Referrals (hidden on team plans), Documentation.
- Quick-add rows: Add words, Add snippet, Add formatting rule.
- Settings rows: General (⌘,), Hotkeys, Dictation, Account, Commands, Built-in tools, Agents (beta), MCP servers, Command line (beta), and a theme toggle.
- Synonyms: "personalization" → Formatting, "tools" → Built-in tools, "mcp" → MCP servers.
- Transcript search: at least 2 characters, up to **15 results**, and picking one opens it in Activity.
- Keys: ↑↓, Enter, Esc.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
