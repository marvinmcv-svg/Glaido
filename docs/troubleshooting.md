# Troubleshooting

This page covers the problems you are most likely to hit with the Vocalis desktop app, and the limits we know about. Where something doesn't work yet on a platform, we say so plainly rather than hiding it.

If none of this helps, use **Report a bug** from the account menu and describe what you expected and what happened.

**Contents**

- [Your recordings are safe](#your-recordings-are-safe)
- [macOS: permissions](#macos-permissions)
- [macOS: "No text field to paste into"](#macos-no-text-field-to-paste-into)
- [Signing in](#signing-in)
- [Linux: Wayland sessions](#linux-wayland-sessions)
- [Linux: pasting into a terminal](#linux-pasting-into-a-terminal)
- [Linux: no keychain](#linux-no-keychain)
- [Where Vocalis keeps its files](#where-vocalis-keeps-its-files)

---

## Your recordings are safe

Vocalis writes your voice to an encrypted file on your computer **while you speak**, not only when you stop. So if something goes wrong part-way through, the audio is still there and you can turn it into text later.

| What happened                                                        | What you'll see                                                                                                          |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Vocalis crashed or was closed while you were recording               | Next time Vocalis starts, the recording appears in History as a failed dictation ("Vocalis closed before this recording finished"). |
| Your internet dropped for a moment while you were speaking           | Nothing, usually. When you stop, Vocalis sends the whole recording again on a fresh connection and pastes as normal.     |
| Your internet was still down when you stopped                        | Nothing is pasted and your clipboard is left alone. A notice says the server couldn't be reached, and History has a failed entry. |
| You pressed **Esc** to cancel                                        | Nothing is pasted, but the audio is kept in case you want the text after all.                                            |
| The server took too long or couldn't transcribe                      | A failed entry in History with the audio kept.                                                                           |
| You ran out of free words for the week                               | A failed entry in History with the audio kept for later.                                                                 |

**To get the text back:** open Home → Activity, find the entry marked as failed, and press **Retry**. You can also press Retry on the notice that appears when a dictation fails. When it works, the text is saved to History (it is not pasted, because you have probably moved on); use **Copy** to paste it wherever you want.

A few things to know:

- Retry needs the audio. Vocalis keeps audio for your **100 most recent recordings**; older entries keep their text but lose their audio, and can't be retried.
- If the text arrives after you've already moved on (a very slow connection), it goes to History instead of being pasted into whatever you're doing now.
- Recordings stop at **10 minutes**. You get a warning 30 seconds before, and everything you said up to the limit is still transcribed.
- If Vocalis hears only silence, it tells you instead of pasting nothing. Check that the right microphone is picked and isn't muted.

---

## macOS: permissions

Vocalis needs two permissions on a Mac. The setup wizard walks you through both, and you can come back to them at any time from **Settings → Restart wizard**.

### Accessibility

This lets Vocalis notice your shortcut while you're in another app and paste the text where your cursor is. Without it, pressing the shortcut does nothing.

1. In the wizard's Permissions step, press **Open System Settings**. (Or open System Settings → Privacy & Security → Accessibility yourself.)
2. Turn on **Vocalis**. If it isn't in the list, press **+** and choose it from Applications.
3. Go back to Vocalis. The wizard checks every couple of seconds and moves on by itself once the permission is on.

If the shortcut still does nothing:

- Look in Privacy & Security → **Input Monitoring** as well, and turn Vocalis on there if it's listed. Some macOS versions ask for this separately for apps that listen to the keyboard.
- If you updated or moved the app, macOS may still be holding on to the old copy. Remove Vocalis from the Accessibility list with **−**, add it again, then quit and reopen Vocalis.

### Microphone

macOS asks the first time Vocalis listens, which is normally the microphone test in the setup wizard. Choose **Allow**.

If you said no by mistake, open System Settings → Privacy & Security → **Microphone** (the wizard's **Privacy settings** button goes straight there), turn Vocalis on, and quit and reopen Vocalis.

### The Fn (Globe) key

The default shortcut on a Mac is **Fn**: hold it to dictate, or press **Fn+Space** for hands-free. Not every keyboard reports Fn the same way, and some external keyboards don't send it at all. If holding Fn doesn't start a dictation (and Accessibility is on), pick a different shortcut: **Settings → Restart wizard**, go to the First dictation step and press **Change**.

You may also want to set System Settings → Keyboard → "Press Globe key to" (or "Press fn key to") to **Do Nothing**, so macOS doesn't open the emoji picker or switch input source each time you dictate.

---

## macOS: "No text field to paste into"

When you finish speaking, Vocalis asks macOS what currently has the keyboard focus. If nothing does, or the focus is on something you can't type into (a button, a list, a toolbar, the desktop), Vocalis doesn't send a paste keystroke into thin air. Instead you get a notice: **No text field to paste into**.

Your words are not lost. They're saved in History; press **Copy** on the notice (or on the entry in Activity), click into the field you meant, and paste.

To avoid it, click into the text box before you start dictating.

Notes:

- If Vocalis can't tell what's focused (some web pages and custom-drawn apps describe themselves vaguely), it pastes anyway, so a wrong guess never swallows a dictation.
- This check needs the Accessibility permission. Without it, Vocalis can't look and always tries to paste.

### Switching apps while you talk

If you start dictating in one app and switch to another before you stop, Vocalis goes back to the app you started in and pastes there (macOS and Windows; Linux on X11 too). If you'd rather have the text go wherever you are when you stop, turn off **Settings → Paste into the starting app**.

---

## Signing in

Vocalis signs you in through your web browser, so your password is only ever typed into the Vocalis website, never into the desktop app.

1. In Vocalis, press **Sign in** (Settings → Account, or the First dictation step of the wizard). Your browser opens the Vocalis sign-in page.
2. Sign in there (or create an account).
3. On the next page, press **Open Vocalis**. Your browser may ask whether to open the Vocalis app; allow it.
4. Back in Vocalis, your email appears under Account. You stay signed in after restarts; your session is kept in your system's keychain.

If it doesn't work:

- **Nothing happened after "Open Vocalis".** Use the **Open Vocalis** link on that page. It works once and only for five minutes; after that, start again from the app.
- **The app is still waiting.** Press **Open browser again** in Vocalis to get a fresh sign-in page. A sign-in that was started more than 15 minutes ago is no longer accepted, so start a new one rather than reusing an old tab.
- **On Linux, the browser can't open the app.** The browser hands the link to your desktop, which needs to know that Vocalis handles `vocalis://` links. Installing the .deb or .rpm package sets this up; a copy you built or unpacked yourself may not have it registered.
- **You were signed out by yourself.** If your session was ended elsewhere (for example you signed out of all devices on the website), Vocalis notices the next time it refreshes and signs you out. Sign in again.

---

## Linux: Wayland sessions

**Today, Vocalis works fully on Linux under X11.** On a Wayland session, the global shortcut and the automatic paste may not work, depending on your desktop. We'd rather tell you that up front than have it fail silently.

Why: Wayland, by design, doesn't let one app watch the keyboard while another app is in front, and doesn't let one app type into another. That protects you from keyloggers, but it also blocks a dictation app from doing its two main jobs. Vocalis currently listens for its shortcut and sends the paste keystroke through X11. On Wayland that only reaches apps that themselves run through the X11 compatibility layer (XWayland), and many desktops don't pass shortcuts through at all.

What you'll notice on Wayland:

- The setup wizard shows a note that you're on a Wayland session.
- Pressing the shortcut may do nothing while a native Wayland app is in front.
- If dictation does run, the text may not be pasted; it's still saved in History, ready to copy.
- Vocalis can't tell which app you were in, so it can't switch back to the starting app or pick the terminal paste shortcut. Text goes wherever your cursor is.

What you can do now:

- **Log in to an X11 session.** Most distributions offer one on the login screen (often under a gear icon, named something like "on Xorg"). Everything works there.
- If you stay on Wayland, keep Vocalis's main window handy and use **Copy** from History to paste results yourself.

What's coming (planned for the Linux parity phase, P5):

- **Shortcuts** read directly from the keyboard device, with a small system rule that the package installs for you (or that you can allow from inside the app with your password). This will support every shortcut, including modifier-only ones and double-tap.
- Where that isn't possible, the desktop's own **shortcut portal**. Your desktop asks you once to approve each shortcut. It needs a regular key in the shortcut (no modifier-only or mouse-button shortcuts).
- **Paste** through a virtual keyboard device when allowed, or the desktop's **remote-desktop portal** otherwise. GNOME asks you once, and briefly shows its screen-sharing indicator each time Vocalis pastes. If you decline, Vocalis asks again next time and offers Copy instead.

Also worth checking on any Linux session: keyboard options that move or remap keys (for example making Caps Lock act as Ctrl, or a Compose key on Left Ctrl) and layouts that put V somewhere else (such as Dvorak) can affect the shortcut and the paste keystroke. If dictation starts but nothing is pasted, try a plain US layout to see whether that's the cause.

---

## Linux: pasting into a terminal

Most terminals use **Ctrl+Shift+V** to paste, because Ctrl+V means something else to the programs running inside them. When you start a dictation in a terminal window, Vocalis notices and pastes with Ctrl+Shift+V instead of Ctrl+V. This covers common terminals such as GNOME Terminal (and Console), Konsole, xterm, Alacritty, kitty, WezTerm, Tilix, Terminator, foot, Ghostty and others.

If the text doesn't appear in your terminal:

- Your terminal may not be recognised, or may use a different paste shortcut. Check its keyboard settings and make sure Ctrl+Shift+V pastes. If it does and Vocalis still uses Ctrl+V, please report your terminal's name with **Report a bug**.
- On Wayland, Vocalis can't see which app you're in (see above), so it uses plain Ctrl+V.
- Your words are always in History; use **Copy** and paste them yourself.

On macOS (⌘V) and Windows (Ctrl+V) terminals paste with the normal shortcut, so nothing special is needed there.

---

## Linux: no keychain

Vocalis keeps three secrets: the key that encrypts your saved recordings, the key that encrypts your History, and your sign-in session. Normally these go into your system keychain (the "Secret Service" that GNOME Keyring or KDE Wallet provides).

Some setups don't have one running: minimal window managers, some remote or headless sessions, or a keyring that is locked and can't be unlocked. In that case Vocalis still works: it stores those secrets in files inside its own data folder (`~/.local/share/app.vocalis.desktop/keys/`) that only your user account can read (permission `600`), and writes a warning to its log.

This is weaker than a keychain: anything running as your user, or anyone who gets a copy of your home folder, could read those files.

**Switching to a keychain later has a catch today.** Once a keychain is available, Vocalis looks for its keys there only. It doesn't yet move the keys over from the files, so it creates new ones: your existing History and saved recordings can no longer be opened, and you'll need to sign in again. Moving the keys across automatically is on our list. Until then:

- If you can, set up a keychain **before** you start using Vocalis. Install and start a Secret Service provider, for example `gnome-keyring` (with its secrets component) or KWallet, make sure it's unlocked when you log in, then start Vocalis.
- If you've already been using the file fallback and your History matters to you, keep your current setup for now. Copy anything you need out of History before you switch.

Don't delete the files in `keys/` by hand: without them, your existing History and saved recordings can't be decrypted.

A setting that shows which kind of storage is in use is planned for a later release.

---

## Where Vocalis keeps its files

Everything Vocalis saves stays on your computer, encrypted: your History, your settings and your recordings.

| System  | Folder                                              |
| ------- | --------------------------------------------------- |
| macOS   | `~/Library/Application Support/app.vocalis.desktop` |
| Windows | `%APPDATA%\app.vocalis.desktop`                     |
| Linux   | `~/.local/share/app.vocalis.desktop`                |

Inside: `history.db` (your History), `recordings/` (audio, encrypted), `settings.json` (your choices, such as shortcuts, languages and microphone). If `settings.json` gets damaged, Vocalis sets it aside and starts with defaults rather than refusing to start.
