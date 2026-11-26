# Spec 4.8: App shell, settings and onboarding

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.8 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Window and sidebar**
- Sections: dictation features (Home, Formatting, Dictionary, Snippets), then agent and utility features (Commands [Beta badge], Extensions, Insights).
- Footer: **Get a free month** (hidden on team plans), Settings, and the account menu (Tips & shortcuts video, Report a bug, Sign out).

**Settings tabs** (with section labels)
- **General:** Appearance (Dark / Light / System); App language (14 languages, each listed in its own script: English, Deutsch, Español, Nederlands, Français, Italiano, Polski, Română, Русский, Srpski [Latin], Ελληνικά, 日本語, 한국어, 简体中文; or System default); Launch at login; Show in menu bar (mac) / Show tray icon (Windows); Show in Dock (mac; off means a background agent app); Send crash reports; Beta features; About (version, system info, **Check for updates**).
- **Hotkeys:** Dictation (hold), Hands-free dictation, Commands (hold), Hands-free commands, Spotlight (unset), extension shortcuts, Enter to stop and paste. Read-only references: ⌘, (Settings) and ⌘K (Palette). Hotkey settings are **stored per platform**.
- **Dictation:** Microphone (§4.10), Include Bluetooth devices, Input level, Language(s), Behavior (Copy to clipboard, Paste into starting app, Mute background, Interaction sounds, Background audio mode), On screen (Dictation bar location).
- **Account:** Name and email, Manage account (opens the web app), **Manage plan** (portal or checkout), billing-channel notice (App Store or Google Play subscriptions are managed there), Setup wizard (Restart wizard), Permissions (**Check permissions**), Clear history, Share app logs, Sign out.
- **Commands:** walkthrough video ("see it in action"), command window style and position.
- **Built-in tools**, **MCP servers**, **Agents** (beta), **Command line** (Developer section).

**Hotkey picker**
- Press modifiers then the key, then Save.
- **Unset** leaves a slot empty.
- **Double-tap** toggle for single-key bindings.
- **Mouse buttons** (extra buttons) can be bound.
- Left and right modifiers are distinct, except Shift, which stays generic.
- Modifier-only hotkeys are allowed (Fn, Right ⌥, Right Ctrl).
- Supported keys: letters, digits, symbols, Space, arrows, F-keys (Windows F1–F12; macOS above F12 too).
- **Conflicts:** an internal clash asks whether to move the binding (two hotkeys can't share). On Windows, warn about OS-risky combos (Win+Space, bare AltGr, single bare modifiers, Alt+Tab) but allow "save anyway". macOS shows no warnings.

**First-run setup wizard**
1. **Permissions** (macOS: Accessibility and Microphone; Windows and Linux: microphone; the step auto-advances once granted).
2. **Mic test**, with device picker and live meter.
3. **Languages** (defaulting to the system locale).
4. **First dictation**, with the hotkey shown and editable.
5. ★ An optional **import** from other tools (§4.5.4).

It can be re-run from Account.

**Notifications catalog (native OS notifications):**
- Weekly limit reached (with Upgrade).
- Dictation failed (with Retry).
- Nowhere to paste / Could not paste (with Copy).
- Dictionary suggestion (with Add).
- Mic silence detected (switch mic?).
- Command finished in the background (opens the record).
- Couldn't start (Retry / Get help).
- Update required (too old for the service).
- Slow connection.

**Updates.** Background check and download. **Install only when not dictating**, restart, and come back quietly if the window wasn't focused. Manual "Check for updates". Linux updates may prompt for the password once. A minimum-version gate shows **Update required**.

**Support and diagnostics**
- **Report a bug** (account menu): a free-text description, with "Include app logs" off by default. The report still sends even if the logs fail.
- **Share app logs** (Account): confirm first, then send system info plus **7 days** of the activity log. **Never dictated text, dictionary, snippets or file paths.**
- **Crash reports:** sent on the next launch after a crash or failed start, with that day's log. On by default with an opt-out. Retained **30 days**, stored in the EU.
- **Retention:** bug reports **12 months**, log bundles **30 days**.

**In-app video tutorials:** a Commands walkthrough (~3 min) and Tips & shortcuts (5 tips, under 1 min), also embedded in the docs.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
