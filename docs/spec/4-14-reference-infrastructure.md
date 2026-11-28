# Spec 4.14: Reference infrastructure (observed from public policies; use it for inspiration, not imitation)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.14 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

- Speech: their **own fine-tuned Whisper-class model** hosted on Baseten, plus Groq. Infra on **AWS and Hetzner**. **Cloudflare** for CDN, DNS, WAF and geo.
- Web search and research tools: **Exa**. Transactional email: Lettermint. Marketing email: GoHighLevel.
- Payments: Polar (MoR), App Store, Google Play. Analytics: GA4 (consent-gated). Affiliates: Dub. Ads: Meta Conversions API, server-side. Trust: Vanta.
- The desktop app is native with a WebView UI (WebView2 on Windows), consistent with **Tauri + Rust**. The Linux binary is named `*-core`.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
