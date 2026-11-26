# Spec 4.10: Microphone and platform specifics

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.10 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Microphone selection**
- **Auto-detect** (recommended). Priority on macOS:
  1. A Bluetooth system default, if Bluetooth is allowed.
  2. A wired external device whose name contains earpods, headset, headphones or usb (not the built-in mic).
  3. The system default (skipping Bluetooth when not allowed).
  4. The previous pick, if still connected and external.
- **Never chosen:** iPhone and iPad **Continuity** mics (they wake the phone). **Virtual and aggregate devices** (BlackHole, Soundflower, Krisp) are hidden unless **Show virtual microphones** is on, and even then auto-detect never uses them.
- The dropdown shows "Auto-detect (Device name)".
- Windows: the system default, falling back to the first device. The Bluetooth toggle has no effect there.
- **Bluetooth:** excluded from auto-detect by default (slower start, lower quality). The toggle shows only in auto mode. Picking a Bluetooth device manually shows a "Use Bluetooth microphone?" warning with **Use anyway**. If the only mic is wireless (AirPods), use it rather than failing.
- **Lid closed** on a Mac: switch away from the built-in mic. **Silence detection:** if a mic sends only silence, offer to switch.
- Multi-input audio interfaces: capture the voice channel without loopback bleed.
- **Noise reduction is always on** (no setting).

**macOS:** Accessibility (global hotkeys, paste, selected text via AX API) and Microphone permissions. Fn and Globe key capture. Menu bar icon. Login item. Apple Silicon and Intel, macOS 13+.

**Windows 11:**
- Low-level keyboard hook.
- SendInput paste.
- UI Automation for the selection.
- Tray icon, which Windows may hide in overflow.
- The installer bundles the **WebView2** runtime if missing.
- Microphone privacy toggle.
- Defaults: dictation Ctrl+Win, hands-free Ctrl+Win+Shift, commands Left Ctrl + Left Alt.

**Linux (x86_64, Wayland and X11):**
- **Hotkeys:**
  - Direct route: read `/dev/input` (evdev). The **.deb installs the udev rule** `70-…-keyboard.rules` (`TAG+="uaccess"` for keyboards), and input-group membership also works. Supports any hotkey, including modifier-only and double-tap.
  - Fallback route: the XDG **GlobalShortcuts portal** (GNOME asks once per shortcut and again when one changes; it needs a regular key, with no modifier-only or mouse hotkeys; GNOME consumes the shortcut).
  - The .rpm doesn't install the rule. In-app **Allow keyboard access** installs it via a polkit password prompt. Switching to the direct route happens within seconds, with no restart.
- **Paste:**
  - Clipboard plus Ctrl+V via a **uinput virtual keyboard** when `/dev/uinput` is writable.
  - Otherwise the **RemoteDesktop portal** (GNOME asks once, and its indicator flashes on each paste). If denied, ask again next time and show "Could not paste" with Copy.
- Env overrides: `Vocalis_PASTE_BACKEND=portal`, `Vocalis_HOTKEY_BACKEND=portal`.
- Troubleshooting covers xkb options that remap Left Ctrl (`compose:lctrl`, `ctrl:swapcaps`) and non-US layouts (Dvorak) that move V.
- Defaults: dictation Left Ctrl+Space, hands-free Left Ctrl+Shift+Space.
- Download: the site can't tell distros apart, so the user picks .deb or .rpm after signup.

**Installation and system requirements:**
- **macOS 13+** (Apple Silicon and Intel): a `.dmg`, drag the app to Applications, and the setup wizard opens on first launch.
- **Windows 11**: a signed installer that bootstraps WebView2.
- **64-bit Linux**: `.deb` (Debian/Ubuntu) or `.rpm` (Fedora/RHEL).
- **iOS 18+**: App Store. **Android**: Play Store.
- Every platform needs a working microphone and an internet connection (★ except in offline mode).
- The marketing site detects the OS, and the download comes after signup.

**Uninstall:** standard OS removal. The account and subscription are untouched; sign in again after reinstalling.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
