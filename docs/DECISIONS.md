# Architecture decision records

Format: context → decision → consequences. Status is one of *proposed*, *accepted*, *superseded*.

## ADR-0001: Product variables use the master-prompt defaults — accepted (2026-11-26)
**Context.** §0 of the master prompt asks for variables before work starts. The human said "go ahead" without overrides.
**Decision.** `PRODUCT_NAME` = Vocalis, `WAKE_WORD` = Vocalis, `DOMAIN` = vocalis.app, `CLI_NAME` = vocalis, `ACCENT` = electric coral `#FF5A36`, `COMPANY` = Your Company Ltd., `STT_PROVIDER` = groq (whisper-large-v3-turbo), `LLM_PROVIDER` = anthropic, Pro = $18/user/month or $15/user/month billed annually, free = 2,000 words a week, `PRIMARY_OS` = macOS.
**Consequences.** Names are centralized in `packages/config/product.json` and `crates/core/src/product.rs` so a rename touches two files. Open question O1 in `docs/PLAN.md`.

## ADR-0002: Monorepo with pnpm workspaces, Turborepo and a Cargo workspace — accepted
**Decision.** Follow the §5.1 layout. JS packages under `apps/*` and `packages/*`; Rust crates under `crates/*` plus `apps/gateway` and `apps/desktop/src-tauri`, all in one Cargo workspace.
**Consequences.** One lockfile per ecosystem; CI caches both.

## ADR-0003: The gateway is written in Rust (axum) — accepted
**Context.** §5.1 allows Rust axum or Node/Hono.
**Decision.** Rust axum. The deterministic formatting stages (snippets, dictionary post-pass, tags, style transforms, wake word) live in `crates/core` and run identically on the desktop (offline/raw paths) and in the gateway.
**Consequences.** One implementation of the formatting rules, one test suite. Tower middleware gives us the no-body logging guard.

## ADR-0004: Desktop UI is React + Vite + TypeScript inside Tauri 2 — accepted
**Decision.** Tauri 2 shell; React 19 with Vite; styling from `packages/design-tokens` CSS variables. Typed IPC through Tauri commands (specta bindings once the command surface stabilizes in P1).

## ADR-0005: Clean-room process — accepted
**Decision.** The Research Analyst writes `docs/spec/*` and `docs/research/*`; implementers build from those only. A CI step (`scripts/no-reference-strings.sh`) fails if the reference trademark or its distinctive assets/colors appear outside `docs/`.
**Consequences.** `docs/MASTER_PROMPT.md`, `docs/spec/` and `docs/research/` are the only places the reference is named.

## ADR-0006: Billing provider — proposed
**Options.** Stripe Billing + Stripe Tax (default in §5.7) vs. a merchant of record (Paddle, Polar) for VAT handling.
**Decision due.** Before P4. Human input requested (PLAN O4).

## ADR-0007: Gateway auth verifies Supabase JWTs via JWKS, with optional legacy HS256 — accepted (amended 2026-11-26)
**Context.** The first version verified HS256 with the project secret. Testing against a real local Supabase (CLI 3.4, Postgres 17) showed access tokens are now signed with **ES256** and a `kid`, published at `/auth/v1/.well-known/jwks.json`; hosted projects are moving the same way.
**Decision.** `apps/gateway` verifies `Authorization: Bearer` tokens with keys from the project's JWKS (`SUPABASE_URL`), cached in memory and re-fetched on an unknown `kid` at most every 30 s. HS256 is accepted only if `SUPABASE_JWT_SECRET` is set (legacy projects). The key, not the token header, decides the algorithm. Required claims: `exp`, `sub`, `aud = authenticated`. `x-device-id` is read as an optional header.
**Consequences.** Verified by `apps/gateway/tests/jwks.rs` (runtime-generated P-256 keys, no private key in the repo) and manually against local Supabase.

## ADR-0008: Zero-retention logging is enforced by construction — accepted
**Decision.** The gateway's request-logging middleware only ever sees method, matched route, status, latency and byte counts; it never reads bodies or query strings. A test (`apps/gateway/tests/no_body_logging.rs`) sends a request carrying a sentinel transcript and asserts the sentinel never appears in captured log output.

## ADR-0009: Phase R runs from the audit until runtime REA access exists — accepted
**Context.** REA runtime captures (R1, R2) need a desktop browser and a personal account; this cloud container has neither for the reference.
**Decision.** Seed `docs/research/finding-ledger.md` from the dated audit in the master prompt, marked *observed (audit)* with medium confidence. Re-verify rows with REA when access exists (PLAN O5). R3 (open-source analogs) is ordinary repo reading and proceeds now. R4/R5 apply to our own builds from P1.

## ADR-0010: Pipeline and gesture choices where the spec is silent — accepted
**Context.** Spec 4.1 and 4.5 leave a few orderings open (rule 3 of §1: mark UNKNOWN, decide, record).
**Decisions.**
1. **Snippets fire even when Raw text is on.** A whole-dictation trigger is explicit intent; this keeps snippets usable in terminal rules (the Terminal preset turns Raw on). §5.4 step 1 is amended accordingly.
2. **User dictionary spellings survive the Lowercase style.** Lowercasing runs before the dictionary post-pass, and tag expansions (e.g. a clipboard URL) are never re-cased.
3. **Esc cancels push-to-talk as well as hands-free sessions.**
4. **Recording starts on the first key-down**, even when double-tap is enabled, to protect the < 80 ms start budget. A tap that turns out not to be part of a double tap produces a `Discard` action, so no record is created.
5. **Wake word position 2 accepts any first word** ("hey", "ok", "hi" …), and a possessive ("Vocalis's") counts.
6. **Casual style** drops only the final full stop of the text on the deterministic path; the LLM path follows the style block in the prompt.
**Consequences.** Covered by unit tests in `crates/core` (`formatting`, `gesture`, `wake`).

## ADR-0011: Dictation transport, providers and cleanup model — accepted (2026-11-26)
**Context.** P1 needs `WS /v1/dictate` (§5.3) with Groq batch-on-stop, a Deepgram adapter and LLM cleanup (§5.4).
**Decisions.**
1. **PCM16 over WebSocket for the MVP**, not Opus. 32 KB/s is fine on broadband and avoids a codec on both ends; Opus is a later optimization behind the same messages.
2. **Retries use the same WebSocket session** (start → audio → stop) with the decrypted local journal, so one code path serves live and retried dictations. `POST /v1/dictate/retry` stays a stub and will be removed if nothing needs it by P4.
3. **STT failover** walks `STT_PROVIDERS` in order on retryable errors (timeouts, 429, 5xx, network); a 4xx stops the chain.
4. **Cleanup model `claude-haiku-5-5`**, as the brief asks for a fast Haiku-class model for formatting (`ANTHROPIC_CLEANUP_MODEL` overrides). Raw HTTP, since Rust has no official SDK. `output_config.effort = "low"` with adaptive thinking for latency; no sampling parameters (Haiku 5.5 rejects non-default ones); `anthropic-version: 2026-10-21`.
5. **Cleanup never blocks dictation.** A refusal, `max_tokens` stop, error or 8 s timeout falls back to deterministic formatting (`formatting::fallback`), and the reply says `cleaned: false`.
6. **Wire tolerance:** every field of `app` and `tags` is optional on the wire.
**Zero retention.** Logs carry durations, sizes, provider names and the plan kind only, verified by `tests/dictate_logging.rs`. Provider-side zero retention is a contract setting with each provider (Groq, Deepgram, Anthropic) and must be confirmed before launch; no retention-excluded features (e.g. message threads, batches, files) are used.

## ADR-0012: Desktop sign-in is a PKCE handoff through the web app — accepted (2026-11-27)
**Context.** P1 asks for "sign-in via the web app deep link" (§5.6 `/auth/desktop-callback`). The desktop app must end up with its own Supabase session without ever seeing the user's password, and every sign-in method the web app supports (email, magic link, OAuth) must work.
**Decision.**
1. The app makes a PKCE verifier (RFC 7636, S256) and a random state, and opens `/auth/desktop?challenge=…&state=…`. Signed-out users go through `/login` and come back.
2. The signed-in user clicks **Open Vocalis**. A server action stores `sha256(code)` with the challenge and user id in `desktop_handoffs` (service role only: RLS on, no policies, no grants to `anon`/`authenticated`) and the browser follows `vocalis://auth/callback?code=…&state=…`.
3. The app checks the state against its pending sign-in (single use, 15 minutes) and POSTs `{ code, verifier }` to `/api/desktop/exchange`. The route deletes the row (single use, 5 minutes; a wrong verifier still burns it), checks the verifier, and mints a **separate** session: an admin-generated magic-link token (never emailed) verified by a throwaway anon client. It returns the tokens plus the public Supabase URL and anon key.
4. The refresh token is stored in the OS keychain (user-only file fallback, as for the encryption keys). The app refreshes 2 minutes before expiry; a 400/401 from the refresh endpoint signs this device out. Sign out revokes only this device's session (`scope=local`).
**Why not OAuth straight from the app?** It would skip email/password users and anyone already signed in on the web. **Why not hand over the browser's tokens?** Refresh-token rotation would sign one of the two out, and a deep link can be observed by other local apps; with PKCE an intercepted code is useless.
**Needs.** `SUPABASE_SERVICE_ROLE_KEY` in the web app's server environment (never `NEXT_PUBLIC_`). `VOCALIS_DEV_TOKEN` still overrides sign-in for development and E2E.

## ADR-0016: Importers and dictionary suggestions — accepted (2026-11-28)
(Number chosen to stay clear of parallel P2 work; renumber on merge if needed.)
**Context.** P2 ships importers (spec 4.5.4) and auto-suggestions on every OS (spec 4.5.1).
**Decisions.**
1. **`crates/import` is pure.** It takes bytes and the current library and returns an `ImportReport` (to add / skipped with a reason / converted). The desktop reads the file and writes through `Store::dictionary_add` / `snippet_add`, so core validation and duplicate rules apply exactly once. Preview and apply both re-parse, so apply always de-duplicates against the library as it is then.
2. **Libraries:** `csv` (MIT/Unlicense), `yaml-rust2` (MIT/Apache-2.0; `serde_yaml` is unmaintained) and `zip` (MIT, `deflate` only). Zip reads are capped (20 MB unpacked, 10,000 items) regardless of header sizes.
3. **Placeholders map only where the meaning matches** (clipboard, current date/time; cursor markers are dropped). Anything else skips the snippet with the reason, rather than importing a snippet with a hole in it. Unknown brace or `%` text is kept literally so code and "50% off" survive.
4. **Formats we can't see are not guessed** (ledger IMP-05, IMP-06): Wispr Flow's documented import layout and a conservative CSV for Superwhisper; no private app storage is read.
5. **Suggestions** (`vocalis_core::suggest`): word-level LCS diff of pasted vs. edited text; a fix is a replacement inside a changed run of at most three words, spelled within 40% edit distance (or case-only), target ≥ 4 chars; edits that keep under half the pasted words are rewrites and ignored. Pairs are keyed case-insensitively; 2 fixes within 30 days → suggestion; dismissals are permanent. State stores only the word pair and timestamps, so it is a plain JSON file (`suggestions.json`).
6. **Field read-back** is a `FieldTextReader` trait in `crates/context` with a stub on every OS for now (AX / UIA / AT-SPI later); History edits feed suggestions meanwhile (documented in `docs/troubleshooting.md`).

## ADR-0017: Formatting rules storage, presets and evals — accepted (2026-11-28)
**Context.** Spec 4.5.3 and the P2 exit checks leave a few choices open.
**Decisions.**
1. **Local storage.** Rules live in the encrypted history database (migration v3, `formatting_rules`), tombstoned like the dictionary, with uuid ids so they can sync to `public.formatting_rules`. Apps are stored as `{id, name}` so a rule still shows "Slack" after the app is gone; the pipeline only sees ids. The table is seeded lazily with `RuleSet::default()`. Every write validates the whole set with `RuleSet::validate`, so the gateway never receives a set it rejects. All apps can be neither switched off nor deleted; Email can be switched off but not deleted (enforced in the store, since `validate` can't see deletions).
2. **Presets are data in `crates/core::presets`**, with app ids per OS; only the current OS's apps are added. Apps and sites another rule owns are skipped and named back to the user. A preset that would claim nothing is not added. The **Email preset fills in the built-in Email rule** (re-enabling it and adding mail apps) instead of creating a second email rule; an existing custom Email prompt is kept. Friendly filter targets the same AI chat apps as the AI chat preset, so with both only the first added claims them.
3. **Precedence** stays as core defines it: a matching enabled site rule beats an app rule (the most specific domain wins), prompts are All apps then the winning rule only, and switched-off rules fall through to the next match, then All apps.
4. **Most used apps** come from records of the last 90 days grouped case-insensitively by app id; browsers and Vocalis are dropped in the store, "still installed" on the desktop. Installed apps: Linux `.desktop` files (`StartupWMClass`, else the file name), macOS XML `Info.plist` of bundles in the Applications folders; Windows has no list yet, so Most used is unfiltered there and apps can be added by exe name.
5. **Evals.** `packages/prompts/evals/formatting/cases.jsonl` (generated by `generate.py`) is run by `tools/evals`: the deterministic part of every case (plan, assembled cleanup request, fallback text, post-pass of a canned model answer) runs in `cargo test` with a 95% gate; `vocalis-evals --live` sends the cleanup cases through the gateway's Anthropic client and checks properties of the output, skipping with a message when `ANTHROPIC_API_KEY` is absent.
**Consequences.** The Terminal preset (Standard + Raw) still capitalizes the first letter of a command ("Git push …"), as the Standard style is defined today; an eval case pins this so a change is deliberate.
