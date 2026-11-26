# Spec 4.11: Mobile (Phase 9)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.11 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Shared:** the same account and subscription, **dictionary and snippets synced** both ways, and commands. **No** extensions, global hotkey, CLI or MCP on mobile.

**iOS / iPadOS 18+:**
- A **keyboard extension** with a mic button (tap to start and stop), an **ABC** key to return to the system keyboard, and **Allow Full Access** required (onboarding explains why). The containing app handles sign-in (email, Apple, Google, GitHub) and the mic permission.
- **Action Button control**, "Dictate with `Vocalis`" (a Controls widget). Press, talk, press. If our keyboard is up, insert; otherwise copy to the clipboard.
- **Command mode** in the keyboard: ask ("a short thank-you note"), and the answer appears above the keys. **Insert** puts it in the field, or **replaces the selection**.
- Billing: App Store subscriptions, recognized on desktop with a "managed in App Store" notice.

**Android:**
- A **full IME keyboard**: typing, suggestions and autocorrect on-device; **on-device word learning** (never in password or no-learn/incognito fields; can be switched off).
- **Keyboard clipboard**: copies kept **1 hour** or pinned; never sent; excluded from backups.
- A **Dictate** button (mic permission on first use; mic active only while dictating; **disabled in password fields**).
- **Selection actions:** **Shorter**, **Fix grammar**, plus **user-created custom actions** (stored on-device).
- Commands requests.
- **Local history of 300** recent dictations (time, app, style); excluded from backups, deleted on sign-out, and nothing kept from incognito fields.
- **No ads, analytics or crash SDKs.**
- Google Play Billing.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
