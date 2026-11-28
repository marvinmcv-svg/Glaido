# Vocalis — delivery plan

This plan follows `docs/MASTER_PROMPT.md` end to end. It was written after a full read of that file (§0–§9 and Appendices A–F). Product variables are the §0 defaults and are recorded in `docs/DECISIONS.md` (ADR-0001).

**What we are building:** Vocalis, an original voice-intelligence layer for every app: dictation, voice commands and desktop utilities behind one hotkey. It reaches feature parity with the reference product audited in the master prompt, but it has its own name, identity, code, copy and assets (§2). We are not making a copy.

## Team and owners

The roles in §1 are played by one agent, in sequence. Work is labelled by role in commits and in `docs/PROGRESS.md`.

| Role | Abbrev | First deliverables |
|---|---|---|
| Tech Lead | TL | Repo, ADRs, CI, integration, reviews |
| Research and RE Analyst | RA | Finding ledger, spec refresh, competitor and OSS-analog notes |
| UX/UI Designer | UX | Design tokens, UI primitives, pill, brand sheet |
| Desktop/Native Engineer | DN | `crates/input`, `audio`, `inject`, `context`, `platform`, Tauri shell |
| Frontend Engineer | FE | Desktop UI, webapp, site |
| Backend Engineer | BE | Gateway, Supabase schema and RLS, quotas, billing |
| AI/ML Engineer | AI | STT providers, formatting prompts, evals, command agent |
| Mobile Engineer | MO | P9 |
| Growth/Marketing | GM | P8 |
| QA and Security | QA | Test harnesses, no-body guard, threat model, release gate |

## Sprint breakdown (1 sprint ≈ 1 week of focused work)

| Sprint | Phase | Scope | Owners | Estimate | Exit check |
|---|---|---|---|---|---|
| S1 | P0 + R | Monorepo, CI matrix, tokens, UI primitives, Supabase schema and RLS, gateway `/v1/meta` with JWT and the no-body guard, Tauri shell, spec stubs, ledger seed | TL, BE, UX, DN, RA | 1 wk | §7 P0 exit |
| S2 | P1 | `crates/core` gesture recognizer and formatting pipeline, `crates/audio` capture, resample, VAD and encrypted WAV journal, pill window | DN, AI | 1 wk | Unit tests green; pill demo |
| S3 | P1 | `WS /v1/dictate` with Groq batch-on-stop and a Deepgram adapter; LLM cleanup; macOS hotkeys (CGEventTap) and paste with clipboard restore | BE, AI, DN | 1 wk | Scripted dictation end to end |
| S4 | P1 | Local store (SQLCipher, FTS5, integrity check), History UI, failed records with Retry, 10-min cap, setup wizard, deep-link sign-in | DN, FE | 1 wk | §7 P1 exit (latency, induced failures, 10-app QA) |
| S5–S6 | P2 | Dictionary, Snippets, Formatting rules and presets, sync, Home stats, Activity search, Insights, Command Palette, suggestions, importers | FE, AI, BE | 2 wk | Formatting eval ≥ 95%, sync E2E |
| S7–S9 | P3 | Commands window, selection capture, field kind, wake word, 10 built-in tools | AI, FE, DN | 3 wk | Intent eval ≥ 90%, approval flows |
| S8–S10 | P4 ∥ P3 | Webapp routes, Stripe, quotas, teams, referrals, delete wizard | BE, FE | 3 wk | Playwright checkout → Pro in < 5 s |
| S11–S13 | P5 | Windows and Linux parity | DN | 3 wk | P1 matrix on Win 11, GNOME Wayland, Fedora, X11 |
| S14–S16 | P6 | Extension host and the six built-ins | DN, FE | 3 wk | Calculator, color, process-guard, schedule tests |
| S17–S18 | P7 | CLI, MCP server, custom tools client, Agents beta | DN, AI | 2 wk | MCP contract suite, weather demo < 2 min |
| S13–S19 | P8 ∥ | Marketing site, docs, i18n, SEO | GM, FE | 6 wk (parallel) | Lighthouse ≥ 95, axe clean |
| S20–S26 | P9 | iOS keyboard and Android IME | MO | 6 wk | Two-way sync, store checklists |
| S23–S27 | P10 | SSO, admin console, trust center, SOC 2 readiness | BE, QA | 4 wk | SSO E2E (Okta, Entra) |
| S28+ | P11+ | §4.15 roadmap in the order given in §7 | all | ongoing | per item |

## Risks

| # | Risk | Impact | Mitigation |
|---|---|---|---|
| K1 | macOS needs Accessibility and Input Monitoring permissions; Fn/Globe capture behaves differently by keyboard | Hotkeys fail silently | Permission checklist in the setup wizard; device-flag tests; fallback hotkey suggestions |
| K2 | Wayland blocks global key capture and synthetic input | Linux dictation unusable on some desktops | evdev + udev rule first, GlobalShortcuts and RemoteDesktop portals as fallback, honest troubleshooting docs (§9.8) |
| K3 | Stop → paste p50 < 1.0 s depends on STT and LLM round trips | Misses the §5.9 budget | Groq batch-on-stop, a fast cleanup model, skip the LLM when the transcript is short and clean, regional gateway, latency harness from S3 onward |
| K4 | Cleanup LLM "answers" the dictation or follows injected instructions | Wrong text pasted | Strict system prompt, injection evals in CI, deterministic post-pass |
| K5 | Clean-room contamination (copied copy, assets or behavior from private surfaces) | Legal | RA/implementer split, "no reference strings" scan in CI (`scripts/no-reference-strings.sh`) |
| K6 | Code signing and notarization need paid developer accounts | No installable release builds | Unsigned CI artifacts until the human provides certificates (open question O3) |
| K7 | Cloud CI cannot test real microphones or hotkeys | Gaps in automated coverage | Pure-logic crates with heavy unit tests; virtual-mic E2E harness on self-hosted runners later |
| K8 | Provider zero-retention guarantees depend on contracts | Privacy claim at risk | Provider ZDR flags in config and checked at startup; no-body logging test |

## Open questions for the human

None of these block P0; P0 runs on the §0 defaults.

- **O1. Product variables.** Are the §0 defaults final (name *Vocalis*, domain *vocalis.app*, accent coral `#FF5A36`, pricing $18 monthly / $15 annual, 2,000 free words a week)? A rename later is cheap now and expensive after P8.
- **O2. Hosted accounts.** P1 needs a Supabase project, a Groq API key, a Deepgram key and an Anthropic key. Until then everything runs against local Supabase and stub providers.
- **O3. Signing.** Apple Developer ID, a Windows code-signing certificate and a Tauri updater key pair are needed before any public build.
- **O4. Billing provider.** Stripe Billing + Tax (default) or a merchant of record (Paddle or Polar). The decision is due before P4 (ADR-0006, currently *proposed*).
- **O5. REA access.** Phase R runtime work (R1/R2 captures) needs a machine with a browser and one personal account on the reference web app. The ledger is seeded from the audit until then.

## Environment notes

- This repo is developed in a Linux cloud container. macOS and Windows targets are built and tested by CI (`.github/workflows/ci.yml`), not locally.
- Real-device work (microphone, global hotkeys, paste into other apps) cannot run in the container and is covered by unit tests on pure logic plus CI builds on each OS.
