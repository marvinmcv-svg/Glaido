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

## 2026-11-27 — P1 Dictation MVP, first slice (local branch `p1/dictation`, not pushed)

**Shipped**
- `crates/audio`: streaming resampler to 16 kHz, level meter, energy VAD (noise-floor tracking that handles noisy rooms, hum, long speech and push-to-talk starts), WAV codec, crash-safe AES-256-GCM `.wav.enc` journal, cpal capture.
- `crates/store`: SQLCipher history with trigram search, day grouping, 100-recording audio retention, integrity check, quarantine-and-restore from rotating backups.
- Gateway `WS /v1/dictate`: Groq/Deepgram STT with failover, Claude cleanup (`claude-haiku-5-5`, effort low) with deterministic fallback, wake word/snippet/empty short-circuits, input limits (ADR-0011).
- `crates/input`: chord matching feeding the gesture recognizer; rdev global listener.
- `crates/inject`: paste with clipboard save/restore, per-OS combos.
- `crates/session`: the orchestrator: streaming while recording, journaling, 10-minute cap, silent-recording drop, slow-connection state, timeout, late results to History, failed records with audio, Esc keeps audio, Retry.
- Desktop: engine wiring, live bar with levels and earcons, keychain keys.
- `e2e/desktop/run.sh`: real app + real gateway on Xvfb with synthetic hotkeys, WAV input and mock STT.

**Verified (Ubuntu 24.04 container)**
- 147 Rust tests, clippy `-D warnings`, rustfmt, Prettier, clean-room scan.
- Desktop E2E passes 3/3: Left Ctrl+Space hold → "Hello from the end to end test" pasted into a focused text box, user clipboard restored, encrypted recording kept, no transcript in gateway logs.
- Guards proven to fail when broken: transcript logging in the dictation path.

**Bugs found by tests and fixed:** VAD ignored speech starting on the first frame and treated steady noise as speech; history repair could not open damaged files (and now quarantines instead of deleting); clients omitting optional `app` fields were rejected; X11 clipboard restore lost the contents.

**Not done yet in P1** (see the next entry for what has since shipped)
- Setup wizard (permissions → mic test → languages → first dictation) and sign-in via the webapp deep link (a dev token is used meanwhile).
- History UI (list by day, copy, playback, Retry button), crash-recovery scan for orphaned journals, notifications UI.
- Frontmost-app context (`crates/context`), paste into the starting app, macOS "nowhere to paste" detection.
- macOS and Windows: compile on CI only; Fn/Globe detection, Accessibility prompts and paste need a real Mac/PC.
- Latency harness for the §5.9 budgets (needs real provider keys).

## 2026-11-27 — P1 second slice: History, crash recovery, desktop sign-in

**Shipped**
- **History (FE, DN):** Activity panel on Home: records grouped by day (Today / Yesterday / date), search (≥ 2 characters, failed records excluded), Copy, expandable record with the full text, an audio player and **Download audio** (decrypted in memory, never written to disk in the clear), **Retry** on failed records, per-record **Delete** (★ better than the reference), "Show older" paging, live refresh on `history-changed`.
- **Notices (FE):** in-app toasts for every engine notice with original copy and Retry / Copy actions.
- **Crash recovery (DN):** at startup, any `.wav.enc` without a record becomes a failed `INTERRUPTED` record with its audio kept for Retry; empty journals are removed; unreadable ones are left untouched.
- **Desktop sign-in (BE, FE, DN; ADR-0012):** PKCE one-time-code handoff through the web app (`/auth/desktop`, `POST /api/desktop/exchange`, `desktop_handoffs` table), refresh token in the OS keychain, automatic refresh, sign out, Settings → Account row. `VOCALIS_DEV_TOKEN` still overrides.

**Verified (Ubuntu 24.04 container)**
- 157 Rust tests (new: recovery scan, store delete, PKCE/callback parsing, sign-in → restart → refresh → revoked-session sign-out against a mock HTTP server), clippy `-D warnings`, rustfmt; desktop and webapp Vitest (new history and handoff tests), typecheck, builds; Prettier; clean-room scan.
- Supabase RLS harness on Postgres 16 passes; the new handoff checks fail when a leaky grant + policy is added.
- Desktop E2E passes: dictation pasted and clipboard restored, then **`kill -9` mid-recording → restart → the audio is recovered as a failed record** (P1 exit: no lost audio when the app is killed).
- History UI rendered in Chromium against mocked engine commands (grouping, failed-record copy, Retry, search).

**Not verified**
- The web half of sign-in against real Supabase: no Docker here for `supabase start`, and the hosted project needs the migration and `SUPABASE_SERVICE_ROLE_KEY` first (runbook step 4). The exchange logic is unit tested with fakes only.
- Deep-link delivery into a running app on Linux needs D-Bus and `xdg-mime`, which this container lacks.

**Still open in P1**
- ~~Setup wizard~~ (shipped, see below).
- Frontmost-app context (`crates/context`), paste into the starting app, macOS "nowhere to paste" detection.
- Induced-failure checks for network drop and screen lock (kill is covered); latency harness for §5.9 (needs provider keys); 10-app manual QA and macOS/Windows hardware checks.

## 2026-11-27 — P1 third slice: setup wizard and saved settings

**Shipped**
- **Setup wizard (FE, DN):** shown on first run and from Settings → Restart wizard. Steps: **Permissions** (macOS Accessibility via `AXIsProcessTrusted`, re-checked every 2 s and auto-advancing once granted; links to the privacy panes; a Wayland notice on Linux), **Microphone** (device picker with the system default, live input meter, "we can hear you" confirmation; nothing recorded), **Languages** (99 languages named via `Intl` in the UI locale and in themselves, accent-insensitive search, at least one kept, defaults to the system language), **First dictation** (current shortcut with **Change**, a practice box, success detected from History, sign-in prompt if needed). Skip is always available.
- **Hotkey picker (DN):** `ChordRecorder` records through the global listener, so Fn and modifier-only shortcuts work; Esc cancels; invalid combinations get the existing chord errors.
- **Settings file (DN):** `settings.json` (languages, microphone, per-platform hotkeys, setup done), atomic writes, damaged files set aside, hand-edited values sanitized. The engine reads languages and the microphone at every start (an unplugged saved mic falls back to the default) and rebuilds its hotkeys live.
- **macOS:** `Info.plist` with `NSMicrophoneUsageDescription`; without it macOS refuses microphone access.

**Verified (Ubuntu 24.04 container)**
- 165 Rust tests (new: key-name and chord round trips, recorder, settings defaults/validation/recovery), clippy, rustfmt; desktop Vitest (languages, chord display), typecheck and build; Prettier; clean-room scan.
- Desktop E2E 3/3 with a saved non-default shortcut (F8): it dictates and pastes, the old default no longer starts anything, and kill-9 recovery still works.
- Every wizard step rendered and clicked through in Chromium against mocked engine commands; the right settings commands are sent.

**Not verified**
- Live hotkey recording and live hotkey changes in the running app (unit-tested parts only; driving Tauri commands from the E2E isn't wired up).
- Microphone test and device list: the container has no audio devices. Accessibility check and privacy-pane links: macOS only, built by CI but not run.

## 2026-11-28 — P1 fourth slice: frontmost-app context and paste into the starting app

**Shipped**
- **`crates/context` (DN):** the app and window in front when a dictation starts (macOS NSWorkspace; Windows foreground window + process image; X11 EWMH active window, falling back to the input focus). A token-based **field-kind** heuristic (terminal, code, email, chat, document, generic) over bundle id / exe / WM_CLASS and name, plus URL hosts for browsers (URLs themselves arrive in P3).
- **Paste into the starting app (spec 4.1):** if the user switched windows while speaking, the starting app is brought back (NSRunningApplication; Windows `SetForegroundWindow` with `AttachThreadInput`; X11 `_NET_ACTIVE_WINDOW` plus `SetInputFocus`), then the text is pasted. On by default; Settings → "Paste into the starting app".
- **Nowhere to paste (macOS):** via the Accessibility API, no paste and a notification with Copy when nothing is focused or the focus is a control that never takes text (buttons, lists, windows…). Unknown roles still paste, so a wrong guess can't swallow a dictation.
- **Terminal paste on Linux:** dictations started in a terminal paste with Ctrl+Shift+V.
- History rows now show the app; the gateway receives the app name, id and field kind (never window handles).

**Verified (Ubuntu 24.04 container)**
- 174 Rust tests (new: paste plan, field kinds for the ten QA apps and more, URL hosts, no handles in serialized context); clippy `-D warnings` for Linux, and for the context crate cross-checked for `aarch64-apple-darwin` and `x86_64-pc-windows-msvc` (type-checked only; CI links and bundles on real macOS and Windows).
- Desktop E2E 3/3: start dictating in one window, switch to another mid-speech, release: the text lands in the starting window and the other stays empty. With the setting off, the same run fails as it should.

**Not verified**
- macOS and Windows backends at runtime (refocus, Accessibility "nowhere to paste"); they need real machines.
- Linux terminal paste in a real terminal; Wayland sessions get no context (pastes go where the cursor is).

## 2026-11-28 — P1 fifth slice: network drops

**Shipped**
- **Resend after a dropped connection (DN):** if the gateway connection drops mid-recording (or never opens), the whole recording is already in the encrypted journal, so at stop it is sent once more on a fresh connection within the same 25 s deadline. If the network is back, the text is pasted as usual; if not, the dictation becomes a failed `NETWORK` record that keeps its audio for Retry. Timeouts and server errors are not resent.

**Verified (Ubuntu 24.04 container)**
- 176 Rust tests (new: one drop then success pastes and resends the full recording; a persistent failure resends exactly once and keeps the audio).
- Desktop E2E against the real gateway, which is killed with `kill -9`:
  - **dropped mid-recording, back before release:** the dictation is still pasted;
  - **down through release:** nothing pasted, clipboard untouched, one failed record, its audio kept (4 recordings on disk at that point);
  - plus the earlier checks (custom shortcut, paste into the starting app, kill-9 recovery). 4/4 passes after replacing a fixed start-up sleep with a wait for "dictation engine ready" (one earlier cold-start run had been flaky).

**P1 exit: induced failures.** Covered: killing the app mid-recording, dropping the network (both ways). Still open: **screen lock**; on Linux it needs a session with a screen locker, so it's a manual QA item for each OS.
