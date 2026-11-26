# Spec 4.0: Positioning (rewrite this in our voice)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.0 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

- **Category:** a "voice intelligence layer" for every app. Dictation, voice commands and desktop utilities, in one hotkey-driven app.
- **Core promise:** speak naturally, and get clean, ready-to-send text in whatever app has the cursor, at roughly 3× typing speed (the reference claims ~150–160 WPM, "save 20+ hours a month").
- **Primary audience:** people who live at the keyboard, especially developers prompting AI agents (Claude Code, Cursor, Codex, ChatGPT). Secondary audiences: email-heavy professionals, team chat users, and multilingual users.
- **Pillars:** works everywhere · private by design (zero retention, local history, no training) · your native language (100+) · customizable per app · does things, not just writes (Commands, tools, extensions).
- **Platforms:** macOS 13+ (Apple Silicon and Intel), Windows 11, 64-bit Linux (.deb/.rpm, Wayland and X11), iPhone/iPad (iOS 18+), Android.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
