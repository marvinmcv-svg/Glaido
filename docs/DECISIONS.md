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

## ADR-0007: Gateway auth verifies Supabase JWTs (HS256 shared secret now, JWKS later) — accepted
**Context.** Local Supabase issues HS256 tokens signed with the project JWT secret; hosted projects can use asymmetric keys via JWKS.
**Decision.** `apps/gateway` verifies `Authorization: Bearer` with `SUPABASE_JWT_SECRET` (HS256), requires `aud = authenticated`, and reads `sub` plus an optional `x-device-id` header. A JWKS verifier is added when the hosted project switches to asymmetric keys.

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
