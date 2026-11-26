# Progress log

Each entry: what shipped, demo steps, metrics, known gaps.

## 2026-11-26 — Phase R seed + P0 Foundations

**Shipped**
- **Docs (RA, TL):** `PLAN.md` (sprints, risks, open questions O1–O5), ADR-0001…0010, 16 spec stubs in `docs/spec/`, finding ledger (19 rows, Q1–Q10 *partial* pending REA access), competitor and OSS-analog notes, parity checklist.
- **Monorepo (TL):** pnpm + Turborepo + Cargo workspace, Prettier, rustfmt, Renovate, product constants (`packages/config/product.json`), clean-room scan (`scripts/no-reference-strings.sh`).
- **`crates/core` (DN, AI):** gesture recognizer (hold, hands-free, double-tap toggle, double-tap-and-hold, Esc, Enter), formatting pipeline (wake word → snippet → raw/LLM → dictionary post-pass), rule resolution with prompt inheritance and site matching, dictionary validation and corrections, tags, cleanup prompt v1.
- **Gateway (BE, QA):** axum; `/healthz`, `/v1/meta`, `/v1/me`, `/v1/dictate/retry` stub; Supabase JWT verification via JWKS (ES256/RS256) with optional HS256; metadata-only access logs with a sentinel test.
- **Database (BE):** Supabase base schema (§5.5) with RLS, column grants protecting plan fields, rule app/site uniqueness trigger; plain-Postgres test harness.
- **Design (UX):** tokens with WCAG AA contrast enforced at build; UI primitives (Button, Card, Switch, StatCard, Keycap, DictationPill); Storybook.
- **Desktop (DN, FE):** Tauri 2 shell: main window, click-through bar window, tray, `vocalis://` deep links, original icon set.
- **Webapp (FE):** Next.js 15 + Supabase SSR: login, signup, magic link, Google/GitHub/Apple buttons, callback, guarded account page.
- **CI (QA):** Rust, JS, DB and 3-OS desktop bundle jobs; release gate with audits.

**Demo**
```bash
pnpm install && pnpm turbo run build
cargo test --workspace                       # 53 core + 9 gateway tests
DATABASE_URL=postgres://… supabase/tests/run.sh   # migrations + RLS checks
npx supabase start                           # local auth stack (Docker)
SUPABASE_URL=http://127.0.0.1:54321 cargo run -p vocalis-gateway   # curl :8787/v1/meta
cd apps/webapp && pnpm dev                   # http://localhost:3001/signup
cd apps/desktop && pnpm tauri dev
```

**Verified in the cloud container (Ubuntu 24.04)**
- All Rust tests, clippy `-D warnings`, rustfmt; all JS typecheck, Vitest and builds; Prettier; clean-room scan; `pnpm audit --prod` clean.
- Guards proven to fail when broken: the no-body logging test (logger leaking the URI) and the RLS test (a leaky policy).
- Migration applied to Postgres 16 (harness) and to local Supabase on Postgres 17.
- Full browser round trip against local Supabase: sign up → account (profile via trigger, read through RLS) → sign out → wrong password → sign in.
- Gateway accepted a real Supabase ES256 access token via JWKS and rejected a tampered one.
- Desktop app launched under Xvfb; the bar toggled, centred and stayed click-through.

**Bugs found and fixed during verification**
- Real Supabase signs access tokens with ES256; the gateway only accepted HS256 (ADR-0007 amended).
- Linux: making the unrealized bar window click-through panicked inside tao; the bar was also positioned with a zero size. Both fixed in `toggle_bar`.

**Known gaps (P0 exit)**
- "Launches on all three OSes" is proven only on Linux. macOS and Windows bundles come from the CI `desktop` job, which has not run yet (the branch isn't pushed).
- No ESLint yet (Prettier + `tsc` only). Add it in P1.
- Bundles are unsigned (PLAN O3). OAuth providers are disabled in `supabase/config.toml` until client IDs exist (PLAN O2).
- Ledger rows are audit-sourced; REA re-verification is pending (PLAN O5).

**Metrics:** none of the §5.9 latency budgets apply yet (no audio path). The bar preview does not record anything.
