# Spec 4.13: Marketing site, docs and growth surface

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.13 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Global**
- Next.js, SSR. Light and dark with a system default (theme-color meta for each).
- **10 locales** (en, de, es, it, pt-BR, nl, ja, fr, hi, ar) with hreflang and `og:locale:alternate`, including **RTL for Arabic**.
- A footer language switcher and Dark/Light/System switch.
- JSON-LD (Organization, WebSite, SoftwareApplication, FAQPage, BreadcrumbList, Article).
- OG images per page. A sitemap of about 780 URLs.
- **Cookie banner**, region-aware via CDN country: Essential / Options / Accept. GA4 loads only with analytics consent. Marketing is off when the browser sends Global Privacy Control. Ad-click attribution uses a **first-party 7-day cookie plus server-side conversion API** (no third-party pixel).
- **OS detection** on the download CTA ("Download for free on macOS / Windows / Linux"), plus an OS chip row (macOS · Windows 11 · Linux · iPhone/iPad · Android). Download CTAs route through signup.
- Footer: Product (Business, Pricing, Extensions, Use cases, Docs), Resources (Blog, Compare, FAQ, Expense, Contact Sales, Support), Company (About, Trust Center, Become an affiliate), Legal (Terms, Privacy, Cookies).

**Home page sections, in order:**
1. **Hero:** headline naming the AI apps it works with, a subhead with the time saved and zero retention, the OS-aware CTA, and OS chips.
2. **"Works in the apps you already use":** an infinite logo marquee (Gmail, GitHub, Notion, Telegram, Perplexity, WhatsApp, Slack, Linear, Cursor, Claude, Figma, Discord, Chrome). Some logos link to use-case pages.
3. **Demo:** "Glide at 150 WPM"-style. A speaker card shows the messy spoken input ("…uh, async await…") next to the clean output. The demo animates.
4. **Three value props:** works everywhere · private by design · speak your language.
5. **Interactive product tour** with tabs (Home, Formatting, Dictionary, Snippets, Commands) rendering live mock UIs: stat cards, an activity list with app icons and a search box, dictionary chips, formatting rules per app.
6. **Extensions grid:** five cards, each with a "Replaces X" tag and a link.
7. **Testimonials carousel:** ours must be real.
8. **Pricing:** Monthly/Annual toggle with a "2 months free" badge, three plan cards (Most popular highlighted), cancel-anytime and taxes notes.
9. **Latest releases:** four newest blog posts.
10. **FAQ accordion (13):** time saved; works for my workflow; ramp-up speed; coding; what's different; is it free; languages; privacy; certifications; OS support; cancel anytime; invoices and VAT; payment methods; refunds.
11. **Final CTA** with a decorative brand pattern.

**Pricing page:**
- Plans, an extensions-included note, an "Expense it" link.
- **Compare plans table** (Dictation / Privacy / Speed and support / Team / Admin groups).
- Security badges.
- An **ROI calculator** with sliders for hours typed per day and hourly value, giving yearly savings and hours back a month (assumes about ¾ of typing time recovered over 20 workdays).

**Other pages**
- **Use cases:** a hub with filters (AI & coding, Team chat, Email, Business, Gaming) and detail pages. Template:
  - a "You say / You get / You need / Works in" summary
  - a table of contents
  - prerequisites
  - why voice
  - the exact formatting-rule setup (a rule card mock with a copyable prompt)
  - numbered steps with per-OS key caps
  - examples (you say → what lands)
  - a built-in-voice vs our-app comparison table with a "checked on" date
  - FAQ
  - a trademark non-affiliation note
  - "Last reviewed" date
  - related use cases
  - a "preferred source" button

  Pages: Cursor, Claude Code, ChatGPT, Slack, Gmail, an accounting app (custom tool), and two games (overlay answers).
- **Comparison (vs) pages:** a hub with filters (Dictation apps, Launchers, Clipboard managers, Text expanders, Color pickers, Keep-awake apps) and detail pages. Template:
  - price and platforms
  - "we cover it with…" chips
  - a feature table grouped Both / They lead / We lead (honest)
  - yearly cost tables with a "checked on" date
  - same / they're ahead / we're ahead / cost
  - a verdict ("pick them if… / pick us if…")
  - an interactive **"add up your stack" cost calculator** (tick the apps you pay for)
- **Extensions hub and pages:** a "Say it" phrase list, feature blocks, a commands table (what it does, what to say, shortcut), "Where it helps" scenarios, details (price, platforms, category, guide link), a **feedback widget**, and an **Open in app** deep link.
- **Docs** (`/docs`): sidebar groups (Getting Started, Videos, Features, Extensions, Command line, Configuration, Personalization, Account, Help), **⌘K full-text docs search** (pages, headings, body), an "On this page" ToC, interactive UI mock figures, per-OS tabs (macOS/Windows/Linux/iPhone/Android), and a **Changelog** with versioned anchors and platform badges. The docs are also the corpus for the "Search documentation" tool and are written to be agent-readable, with "For coding agents" sections.
- **Blog:** categories (Company, Engineering, Guides, Product, Voice AI), author and date, read time, related posts, and a "preferred source on Google" CTA. **About:** mission, team cards, "the deal" (privacy pledges, bootstrapped). **Business**, **Contact**, **Expense**, **FAQ**. **Legal:** Terms, Privacy (GDPR, CCPA/CPRA, CalOPPA, children's privacy, the EU/UK representative, a sub-processor list), Cookies. **Support** page (self-help checklist and an email with a one-business-day response target).
- **Email marketing:** newsletter and lifecycle emails with an unsubscribe link (any ESP, e.g. Loops, Customer.io or Resend).

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
