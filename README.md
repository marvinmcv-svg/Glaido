# Vocalis

Speak naturally and get clean, ready-to-send text in whatever app has your cursor. Voice commands, built-in tools and desktop utilities sit behind the same hotkey.

This repository is the monorepo for the desktop app, the voice gateway, the web app and shared packages. The build plan lives in [`docs/PLAN.md`](docs/PLAN.md), decisions in [`docs/DECISIONS.md`](docs/DECISIONS.md), and progress in [`docs/PROGRESS.md`](docs/PROGRESS.md).

## Layout

| Path                     | What                                                                     |
| ------------------------ | ------------------------------------------------------------------------ |
| `apps/desktop`           | Tauri 2 app (Rust + React)                                               |
| `apps/gateway`           | Zero-retention voice and AI gateway (Rust, axum)                         |
| `apps/webapp`            | Accounts web app (Next.js 15 + Supabase)                                 |
| `crates/core`            | Shared domain logic: gestures, formatting pipeline, dictionary, snippets |
| `packages/design-tokens` | Design tokens → CSS variables and TS                                     |
| `packages/ui`            | Shared React primitives and Storybook                                    |
| `packages/prompts`       | Versioned LLM prompts                                                    |
| `supabase`               | Local config, migrations, RLS tests                                      |

## Getting started

Requirements: Node 22, pnpm 10, Rust stable, Docker (for local Supabase). On Linux, also the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm turbo run build
cargo test --workspace
```

Local services:

```bash
npx supabase start
```

```bash
SUPABASE_URL=http://127.0.0.1:54321 cargo run -p vocalis-gateway
```

```bash
cp apps/webapp/.env.example apps/webapp/.env.local   # fill in the anon key from `supabase status`
pnpm --filter @vocalis/webapp dev
```

```bash
pnpm --filter @vocalis/desktop tauri dev
```

## Ground rules

- Secrets never go in git. Use `.env.local`, the OS keychain or the host's secret store.
- Transcripts, audio, clipboard and history never leave the device except to the gateway for processing, and the gateway never stores or logs them.
- All code, copy and assets are original. `pnpm check:clean-room` runs in CI.
