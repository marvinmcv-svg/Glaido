# Parity checklist

Seeded from master prompt Appendix F. Each item gets a link to its test or demo when checked.

A top-level row is ticked only when every part of it is done on every OS it applies to. Rows touched by P1 have sub-items: a ticked sub-item is shipped and verified as recorded in `docs/PROGRESS.md` (the evidence is linked); an unticked one says what's missing. "Linux E2E" means `e2e/desktop/run.sh` (real app and gateway on Xvfb/X11). macOS and Windows runtime checks are in `docs/qa/p1-manual.md` and have not been run yet.

- [ ] Dictation: hold / toggle / double-tap / Esc / Enter-to-stop / 10-min cap / failover / proxy / crash-safe audio
  - [x] Hold to dictate, end to end: Linux E2E (default and custom shortcut). macOS/Windows: manual QA §5 pending.
  - [x] Hands-free toggle, double-tap variants, Esc, Enter-to-stop logic: unit tests in `crates/core/src/gesture.rs` (`hands_free_binding_toggles`, `double_tap_toggle_*`, `escape_cancels_hands_free_and_hold`, `enter_stops_only_when_enabled`). Not yet exercised in the running app; the Enter-to-stop setting has no UI yet.
  - [x] 10-minute cap with warning: `crates/session/src/tests.rs` `ten_minute_cap_warns_then_asks_to_stop`.
  - [x] STT failover (Groq → Deepgram): `apps/gateway/tests/dictate.rs` `failover_to_backup_then_error_when_all_fail`. Region failover: not built.
  - [ ] System proxy / PAC support: not built.
  - [x] Crash-safe audio, app killed: Linux E2E (`kill -9` mid-recording → failed `INTERRUPTED` record with audio).
  - [x] Crash-safe audio, network drop (brief and through release): Linux E2E against a killed gateway; session tests for one resend.
  - [ ] Crash-safe audio, screen lock: not verified on any OS (manual QA §8). The chord tracker's reset-on-lock is not wired to a lock event yet.
  - [ ] Crash-safe audio, mic disconnect, stall, quota: code paths exist; mic disconnect currently stores the record as "cancelled". Not verified.
- [ ] Bar: waveform, no focus, 3 positions, sounds, background audio modes, input meter
  - [x] Live waveform from engine levels, click-through/no focus: P0 Xvfb check and Linux E2E.
  - [ ] Three positions: not built. Background audio modes: not built.
  - [ ] Start/stop sounds: shipped; not verified with audio output (no devices in CI).
  - [ ] Input level meter: shipped in the setup wizard; not verified (no audio devices in CI).
- [ ] Paste: clipboard restore, copy option, starting app, nowhere to paste, Linux terminal paste
  - [x] Clipboard save and restore: Linux E2E; `crates/inject` unit tests (text, images, empty, keystroke failure).
  - [ ] "Copy to clipboard" option: not built.
  - [x] Paste into the starting app on Linux/X11: Linux E2E (switch windows mid-speech; fails as expected with the setting off). macOS/Windows: manual QA §6 pending.
  - [ ] Nowhere to paste (macOS): shipped; needs a Mac (manual QA §7).
  - [ ] Linux terminal paste (Ctrl+Shift+V): paste-plan unit tests only; not tried in a real terminal.
- [ ] Voice activation (wake word in the first 2 words; name-only prompt)
- [ ] Languages: 100+, multi-select auto-detect, system-locale default
  - [x] Multi-select, at least one kept, system-locale default, accent-insensitive search: `apps/desktop/src/languages.test.ts`; wizard step clicked through in Chromium.
  - [ ] 100+ languages: 99 today.
- [ ] Commands window: stream, context chips, refine, Enter / Esc-to-background, notification, click-to-copy, drag, positions, game overlay
- [ ] Built-in tools ×10 with switches and availability states
- [ ] Custom tools: import, new-server wizard, watcher, policies, approval card (Enter, 60 s), 7 failure types, skills repo, weather demo
- [ ] Agents beta
- [ ] Dictionary: vocab, corrections, tags, 50-char limit, duplicates, search, shift-delete, delete all, suggestions, sync
  - [x] Vocab, corrections, tags, 50-char limit, duplicates, search, Shift-delete, delete all: `crates/store/src/personal.rs` tests, `apps/desktop/src/personal.test.ts`, Chromium check.
  - [ ] Suggestions: logic in `crates/core/src/suggest.rs`; reading the field after paste is a stub on every OS. Sync: not built.
- [ ] Snippets: exact trigger, multiline, tags, sync
  - [x] Exact trigger, multiline, tags, first wins: `crates/core/src/snippets.rs`, `crates/store/src/personal.rs` tests. Sync: not built.
- [ ] Formatting: All apps / Email / custom, 3 styles, raw, 500-char prompt, inheritance, app+site uniqueness, toggles, most-used, migration notice, presets
  - [x] Sections, styles, raw, 500-char prompt, inheritance, uniqueness, toggles, most-used, presets: `crates/store/src/rules.rs`, `crates/core/tests/rule_precedence.rs`, eval suite 81/81 deterministic.
  - [ ] Website matching needs browser URL capture (P3); migration notice not built; Windows installed-apps list not built.
- [ ] Importers
  - [x] Espanso, TextExpander, Alfred, Raycast, Wispr Flow (bulk-import layout), Superwhisper (plain CSV): `crates/import` tests with original fixtures. Vendor export formats for Wispr Flow and Superwhisper are UNKNOWN (ledger IMP-05, IMP-06). Not tried with real exports.
- [ ] Home stats, Activity (chips, search 50, playback, download, retry, report), encrypted storage, 100-audio retention, clear all, Insights
  - [x] Activity list by day, search (50 newest), copy, playback, download, Retry, delete: `crates/store/src/tests.rs` (`search_caps_at_50_newest_first`, `groups_by_local_day_and_pages`, `failed_then_retry_completes`), `apps/desktop/src/history.test.ts`; UI rendered in Chromium against mocked engine commands.
  - [x] Encrypted storage with integrity check and repair: `crates/store` (`corruption_restores_from_backup`).
  - [x] 100-audio retention: `audio_retention_keeps_newest_100_and_failed`.
  - [x] Home stats, Insights, clear all: `crates/store/src/stats.rs` tests, `apps/desktop/src/stats.test.ts`, Chromium check.
  - [ ] Chips: Command/Running shipped; tool count waits for Commands (P3). Report: payload built, not sent (no support endpoint).
- [ ] Command Palette (navigation, quick-add, settings, synonyms, transcript search 15)
  - [x] Navigation, settings rows, theme, synonyms, transcript search (15), keys: `apps/desktop/src/palette.test.ts`, Chromium check.
  - [ ] Quick-add only navigates (no add-form anchor); rows for pages that don't exist yet (Extensions, tools, MCP) are absent.
- [ ] Extensions framework + Spotlight, Color Picker, Clipboard, Text Expansion, Processes, Keep Awake (every rule in §4.7)
- [ ] Settings: all tabs and keys (Appendix B), hotkey picker rules, per-platform hotkeys
  - [x] Per-platform hotkeys saved and applied: settings unit tests; Linux E2E with a saved custom shortcut (F8).
  - [ ] Hotkey picker: chord recorder unit-tested; live recording in the running app (and Fn on a Mac) not verified (manual QA §3). Conflict prompts, double-tap toggle, unset: not built.
- [ ] Setup wizard, check permissions, restart wizard
  - [x] Wizard (permissions → microphone → languages → first dictation), skip, restart from Settings: every step clicked through in Chromium against mocked engine commands.
  - [ ] Check permissions on macOS (Accessibility auto-advance, privacy panes) and the live mic test: need real hardware (manual QA §1, §4).
- [ ] Notifications catalog (Appendix C)
  - [ ] In-app notices with Retry/Copy for failed dictation, nowhere to paste, could not paste, limit near, silence: shipped as in-app toasts (`history.test.ts` maps them). Native OS notifications and the rest of the catalog: not built.
- [ ] Updates (idle install, min version), report bug, share logs, crash reports (retention rules)
- [ ] CLI (all commands, safety semantics, PATH status), MCP server (read-only, approvals, one-click connect)
- [ ] Mic auto-detect rules, Bluetooth, virtual devices, lid closed, silence
  - [ ] Saved microphone with fallback to the system default when unplugged: shipped, settings unit tests only; needs hardware (manual QA §10). Auto-detect rules, Bluetooth, virtual devices, lid closed: not built. Silence: notice shipped, not verified with a real mic.
- [ ] macOS / Windows / Linux specifics (§4.10), packages (.dmg / .msi / .deb + udev / .rpm)
  - [ ] Linux Wayland hotkeys and paste (evdev, portals, udev rule): P5; limits documented in `docs/troubleshooting.md`.
- [ ] iOS keyboard, Action Button, Command Insert; Android IME (typing, learning, clipboard, actions, history 300)
- [ ] Auth providers, magic link, set password, delete wizard, keychain session
  - [x] Desktop sign-in through the web app with PKCE and the refresh session in the keychain: auth unit tests (sign-in → restart → refresh → revoked-session sign-out against a mock server), `apps/webapp/lib/desktop-handoff.test.ts`, RLS harness for `desktop_handoffs`.
  - [ ] Web half against real Supabase, and deep-link delivery into a running app: not verified. Keychain-less Linux falls back to a user-only file (documented in `docs/troubleshooting.md`).
- [ ] Plans, quota, checkout (tax, wallets, localized currency), portal, cancel at period end, app-store awareness
- [ ] Teams, referrals, affiliates, enterprise (SSO, DPA, on-prem), contact form relay, expense page
- [ ] Trust center (controls, sub-processors, updates, NDA access)
- [ ] Marketing site (all sections and pages in §4.13), docs (⌘K, changelog, videos), blog, 10 locales, consent, SEO
- [ ] Privacy guarantees verified (§8)
  - [x] Gateway never logs bodies or transcripts: sentinel tests (proven to fail when broken); Linux E2E greps gateway logs for the transcript.

## P1 exit checks still open

- [ ] 50 scripted utterances within the latency budget (p50 < 1.0 s for short ones): needs provider keys and the latency harness.
- [ ] No lost audio when the screen locks: manual QA §8.
- [ ] Manual QA in 10 apps on macOS and Windows: `docs/qa/p1-manual.md` §5.
