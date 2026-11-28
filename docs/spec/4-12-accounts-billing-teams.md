# Spec 4.12: Accounts, plans, billing, teams, referrals and enterprise

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.12 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Auth.** Google, GitHub, Apple, email and password, and **magic link**. Signup and forgot-password run in the **web app**, opened from the desktop app. The desktop session token is kept in the **Keychain / Credential Manager / Secret Service**. Users who signed up with OAuth can **set a password later**. Account pages are **mobile-responsive**. There is a **self-service delete wizard** at `app.vocalis.app/delete` that issues a reference ID and completes within **48 h**, cascading to the referral and affiliate records.

**Plans** (reference values; set ours in §0)

| | Free ("Basic") | Pro | Enterprise |
|---|---|---|---|
| Price | $0 forever | $20/mo monthly, **$17/mo billed annually ($200/yr, "2 months free")** | Custom |
| Words | **2,000 / week** | Unlimited | Unlimited |
| Every app, 100+ languages, AI auto-edits, Commands (agent), all extensions, all OSes, local history, no server storage | ✓ | ✓ | ✓ |
| Prioritized processing, early access, prioritized support | – | ✓ | ✓ |
| Multiple seats on one bill, owner-managed seats | – | ✓ | ✓ |
| Unlimited members, SSO/SAML, dedicated data processing, API access and integrations, custom billing, DPA/MSA, on-prem option | – | – | ✓ |

- **Quota enforcement:** when the weekly allowance runs out, the *next* dictation fails (audio kept). Show the "Weekly limit reached" notification ("2,000 free words used this week") with **Upgrade**. Show weekly usage in the app. **Trial** is a distinct account state.
- **Checkout:** a hosted checkout from a **merchant of record** (the reference uses Polar; ours is Stripe with Stripe Tax, or Paddle or Polar as MoR). Taxes and VAT are computed at checkout. Major cards, plus **Apple Pay and Google Pay**. **Currency and price localized by country** (CDN geo header). Invoices are in the billing page. **Cancel anytime** and keep Pro until the period ends. No refunds except where law requires (case by case).
- **Plan entry links** carry attribution: `app.vocalis.app/signup?plan=pro&cadence=annual&source=website_pricing`.
- **Manage plan:** the existing subscriber gets the billing portal (view plan, up/downgrade, payment method, history, cancel); others get checkout. Subscriptions bought through the App Store or Google Play are labeled with where to manage them.
- **Teams:** one **owner** invites and removes members and manages seats and billing in the web app. Referrals are hidden on team plans.
- **Referrals:** "Give a month, get a month". Every account has a unique link. The invitee gets **1 free month of Pro**; the referrer gets **1 free month when the invitee pays**, applied to the next payment. Invite by email from the app. A "Past invites" tab shows the statuses **Clicked → Signed up → Qualified**. Rewards apply only to subscriptions purchased through our own checkout.
- **Affiliate program:** partner links with attribution (the reference uses Dub, with a 90-day cookie). We send the partner platform an anonymous account ID and payment amounts for commission, never name or email.
- **Enterprise / business page:** team seats, company SSO (SAML, set up together with the customer), contract (DPA, MSA, custom billing), "words stay yours" (never trained on, never in our database, our own speech model, encrypted in transit and at rest), and procurement FAQ (storage, training, who can see, seats, SSO, DPA, on-prem, EU residency, certifications, how to start a security review).
- **Contact Sales form:** name, work email, company, message, company website. The site posts it to our API, which **emails sales and stores nothing** (transactional email provider).
- **Expense page:** three copyable templates (a short chat note, a manager email, a follow-up) with the price filled in.

**Privacy promises** (these become engineering requirements, see §8):
- The server keeps **no dictation text or audio** after returning a result.
- **No training** on user content, including reports.
- History lives **only on the device**, encrypted.
- Dictation requests carry only what's needed: audio, selected languages, dictionary, snippets, corrections, app version, OS.
- Reports send only the chosen record (never audio).
- Analytics never receive name, email or account ID. **Strip auth and invite tokens from URLs** before measurement.

**Trust Center** (`trust.vocalis.app`, e.g. Vanta, Drata or SafeBase):
- Compliance badges (GDPR, ISO 27001 as the target, SOC 2 Type II in progress).
- Downloadable DPA. Certificate and audit report, and pen-test report, behind NDA request access.
- A live **controls** list (infrastructure, organizational, product, internal procedures, data and privacy).
- A **sub-processor list** with region and purpose.
- An **updates feed** with Subscribe (for example, new sub-processor notices).
- security@ contact and vulnerability disclosure.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
