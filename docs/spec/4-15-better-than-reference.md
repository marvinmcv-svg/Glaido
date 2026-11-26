# Spec 4.15: ★ Where we beat the reference (scheduled in §7 phases P11+ unless noted)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.15 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

These are gaps the reference itself admits on its comparison pages, or limits observed in its docs:
1. **Offline and local STT mode** (whisper.cpp / Parakeet on-device, selectable per device). Also enables an enterprise "nothing leaves the laptop" mode.
2. **Live streaming text** as you speak: partial results in the bar, optionally typed live into the field.
3. **Audio and video file transcription** (drag a file in, get a transcript and summary in History).
4. **Botless meeting notes:** capture system audio and mic locally, then transcript, summary and action items (with consent prompts).
5. **Per-record delete**, auto-expiry policies, and an export of history (JSON or Markdown).
6. **Dictionary auto-suggestions on every OS**, plus **auto-learn names** from contacts or selected text (opt-in).
7. **Shared team dictionary and snippets**, plus admin-pushed formatting rules (Pro Teams).
8. **Bring your own key / choose model** for formatting and commands (Anthropic, OpenAI, local Ollama).
9. **Snippet fill-in fields** (`{cursor}`, `{input:Name}`) and date math tags.
10. **Voice editing commands** inside dictation ("scratch that", "new paragraph", "make that a list").
11. **EU data-residency option** and a HIPAA BAA path (the reference says no residency option).
12. **Optional E2E-encrypted sync** of clipboard and history across the user's own devices.
13. **Extension SDK** (TypeScript, sandboxed, with a permission manifest), opening a path to a community store.
14. **Window management extension** (snap and arrange by voice), since a competitor leads there.
15. **Lifetime license option** and student pricing (a business decision; keep it behind a flag).
16. **Windows 10, Windows on ARM and Linux ARM64** builds.
17. **Accessibility first:** full screen-reader support, high-contrast theme, reduced motion, and keyboard-only operation of every screen.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
