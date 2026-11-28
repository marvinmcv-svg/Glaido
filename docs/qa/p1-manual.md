# P1 manual QA checklist (macOS and Windows)

This is the hands-on part of the P1 exit (master prompt §7 P1): dictation in ten real apps, the screen-lock failure test, permissions, the shortcut picker, pasting into the starting app, "nowhere to paste", switching microphones and the setup wizard. Everything here needs a real Mac or PC with a microphone; the Linux container can't do it.

Run the whole list once on **macOS** and once on **Windows 11**. Tick a box only when the expected result happened. If it didn't, leave it unticked and write what you saw underneath (a screenshot or a short screen recording helps), then file it.

## Before you start

- [ ] Install the build under test from the CI `desktop` job artifacts (macOS `.dmg`, Windows `.msi` or `.exe`). Write down the commit SHA: `________`
- [ ] Use a fresh user account, or quit Vocalis and move its data folder aside so the app starts as if new:
  - macOS: `~/Library/Application Support/app.vocalis.desktop`
  - Windows: `%APPDATA%\app.vocalis.desktop`
- [ ] On macOS, remove Vocalis from System Settings → Privacy & Security → Accessibility, Input Monitoring and Microphone if it's listed, so the permission prompts can be tested.
- [ ] Have a working internet connection, a built-in microphone and a second microphone (a USB or wired headset). Bluetooth headphones are a bonus.
- [ ] Have an account on the Vocalis web app you can sign in with.

**Default shortcuts**

| | Hold to dictate | Hands-free (press to start, press again to stop) |
| --- | --- | --- |
| macOS | Fn | Fn+Space |
| Windows | Left Ctrl + Left Win | Left Ctrl + Left Win + Shift |

**Test sentence** (use it everywhere so results are comparable): _"Hi team, the quarterly report is ready. Can we review it on Thursday at three?"_

Record the OS version, the machine and the microphone for each run:

| | macOS run | Windows run |
| --- | --- | --- |
| OS version | | |
| Machine | | |
| Microphones used | | |
| Tester, date | | |

---

## 1. Setup wizard

- [ ] **First launch opens the wizard.** Expected: the window shows four steps (Permissions, Microphone, Languages, First dictation) with Permissions highlighted.
- [ ] **Skip, then restart.** Press **Skip setup**. Expected: the main window appears. Open Settings and press **Restart wizard**. Expected: the wizard opens again at Permissions.
- [ ] **Back and forward.** Move through each step and back again. Expected: choices you made (microphone, languages) are still there when you return.

### 1a. Permissions step

macOS:

- [ ] Expected: an **Accessibility** row with **Open System Settings**, and a **Microphone** row with **Privacy settings**.
- [ ] Press **Open System Settings**. Expected: System Settings opens on Privacy & Security → Accessibility.
- [ ] Turn Vocalis on and switch back. Expected: within about 2 seconds the row shows **Allowed** and the wizard moves to the next step on its own.
- [ ] Press **Privacy settings** (go Back if needed). Expected: System Settings opens on Privacy & Security → Microphone.

Windows:

- [ ] Expected: only a **Microphone** row (no Accessibility row). **Privacy settings** opens Settings → Privacy & security → Microphone.
- [ ] Turn off "Let desktop apps access your microphone", then continue to the Microphone step. Expected: the mic test shows an error, not a frozen meter. Turn access back on afterwards.

### 1b. Microphone step

- [ ] Expected: macOS asks for microphone access the first time (choose **Allow**). Windows doesn't ask if desktop apps are allowed.
- [ ] Expected: the list shows **System default** plus every connected input device.
- [ ] Speak. Expected: the level meter moves with your voice and a "we can hear you" confirmation appears. Nothing is saved to History.
- [ ] Pick the second microphone. Expected: the meter restarts and follows the second mic (tap each mic to check which one is live).
- [ ] Pick a device, then quit and reopen Vocalis and return to this step. Expected: the same device is still selected.

### 1c. Languages step

- [ ] Expected: the system language is preselected.
- [ ] Search with and without accents (for example "espanol" and "español"). Expected: both find Spanish, shown in your UI language and in its own name.
- [ ] Try to clear every language. Expected: at least one stays selected.

### 1d. First dictation step

- [ ] Expected: the current shortcut is shown with a **Change** button, plus a practice box.
- [ ] If you aren't signed in: expected, a sign-in prompt. Sign in (section 2), then come back.
- [ ] Click into the practice box, hold the shortcut, say the test sentence, release. Expected: the text appears in the practice box and the step shows it as a success.
- [ ] Finish the wizard. Expected: the main window. Restart Vocalis. Expected: the wizard does **not** open again.

## 2. Sign in through the browser

- [ ] Press **Sign in**. Expected: your default browser opens the Vocalis sign-in page; the app says to finish signing in in the browser.
- [ ] Sign in, then press **Open Vocalis**. Expected: the browser asks to open Vocalis (allow it), the page says you're signed in, and the app shows your email under Account within a few seconds.
- [ ] Quit and reopen Vocalis. Expected: still signed in.
- [ ] Sign out from the app, then sign in again but close the browser tab before pressing Open Vocalis. Press **Open browser again**. Expected: a fresh sign-in page, and the sign-in completes.

## 3. Shortcut picker

Open the picker with **Change** on the First dictation step (Settings → Restart wizard → go to the last step).

- [ ] Press **Change**, then press a new combination (for example Ctrl+Shift+D). Expected: it is saved and shown; the old shortcut no longer starts dictation; the new one does, **without restarting the app**.
- [ ] Press **Change**, then **Esc**. Expected: nothing changes.
- [ ] Press **Change** and wait 20 seconds without pressing anything. Expected: a "No keys were pressed" message; the shortcut is unchanged.
- [ ] Modifier-only shortcut: set **Right Ctrl** (Windows) or **Right Option** (macOS). Expected: accepted; holding it alone dictates.
- [ ] **macOS, Fn:** set **Fn** as the shortcut (and test the default Fn+Space for hands-free). Expected: accepted and working with the built-in keyboard. Repeat with an external keyboard if you have one and note whether Fn is seen at all.
- [ ] macOS: if pressing Fn opens the emoji picker or changes the input source, note it; then set System Settings → Keyboard → "Press Globe key to" → Do Nothing and check again.
- [ ] A combination the app rejects (for example a lone letter key, if it is refused). Expected: a clear error message, nothing saved.
- [ ] Quit and reopen. Expected: the custom shortcut survives a restart.
- [ ] Set the shortcuts back to the defaults before section 5.

## 4. Permissions after setup (macOS)

- [ ] With Vocalis running, turn **Accessibility** off for Vocalis. Press the shortcut in another app. Expected: nothing is pasted into other apps and nothing crashes. Note whether the app tells you why.
- [ ] Turn Accessibility back on, quit and reopen Vocalis. Expected: dictation works again.
- [ ] Turn **Microphone** off for Vocalis and try a dictation. Expected: no crash; a failed or empty result rather than a hang. Note what is shown. Turn it back on.

## 5. Dictation in ten apps

For each app: click into a text field, **hold** the shortcut, say the test sentence, release. Then repeat once with **hands-free** (press, speak, press again). Check each expected result:

- the bar appears at the bottom of the screen while you speak, with a moving waveform, and it does not steal focus from the app;
- start and stop sounds play;
- the cleaned-up sentence is pasted at the cursor within a few seconds of stopping;
- whatever you had on the clipboard before is back on the clipboard afterwards (copy a word beforehand, paste it after to check);
- the dictation appears in Home → Activity with the app's name.

| # | App | macOS hold | macOS hands-free | Windows hold | Windows hands-free | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Slack (desktop app), message box | [ ] | [ ] | [ ] | [ ] | |
| 2 | Gmail in the browser, new email body | [ ] | [ ] | [ ] | [ ] | |
| 3 | Notes (macOS) / Notepad or Sticky Notes (Windows) | [ ] | [ ] | [ ] | [ ] | |
| 4 | VS Code, an editor tab | [ ] | [ ] | [ ] | [ ] | |
| 5 | Cursor, an editor tab and the chat box | [ ] | [ ] | [ ] | [ ] | |
| 6 | Terminal (macOS Terminal / Windows Terminal) at a shell prompt | [ ] | [ ] | [ ] | [ ] | Text appears at the prompt and is **not** run |
| 7 | ChatGPT in the browser, prompt box | [ ] | [ ] | [ ] | [ ] | |
| 8 | Notion (desktop app or browser), a page | [ ] | [ ] | [ ] | [ ] | |
| 9 | Microsoft Word, a document | [ ] | [ ] | [ ] | [ ] | |
| 10 | Messages (macOS) / Phone Link messages or WhatsApp desktop (Windows) | [ ] | [ ] | [ ] | [ ] | |

Also, in any one app:

- [ ] **Esc cancels.** Start a hands-free dictation, speak, press Esc. Expected: nothing is pasted; Activity shows a failed "cancelled" entry with its audio and a **Retry** button.
- [ ] **Short tap.** Tap the hold shortcut briefly without speaking. Expected: nothing is pasted and no entry is added.
- [ ] **Silence.** Hold the shortcut for 5 seconds without speaking. Expected: a "We didn't hear anything" notice; nothing pasted.
- [ ] **Long recording (optional, 10 minutes).** Start hands-free and leave it running. Expected: a "30 seconds left" notice at 9:30, an automatic stop at 10:00, and everything said is transcribed.

## 6. Paste into the starting app

- [ ] Click into Notes (or Notepad). Start a **hands-free** dictation, say the test sentence, switch to another app (for example the browser) with Cmd+Tab / Alt+Tab, then stop the dictation. Expected: Vocalis brings Notes back to the front and the text lands there; the other app gets nothing.
- [ ] Repeat with **hold**: hold the shortcut, switch apps with the mouse while still holding, release. Expected: the same.
- [ ] Turn off **Settings → Paste into the starting app** and repeat. Expected: the text is pasted into the app you switched to.
- [ ] Turn the setting back on.

## 7. Nowhere to paste (macOS)

- [ ] Click on an empty area of the desktop (Finder in front, no text field focused). Dictate the test sentence. Expected: nothing is typed anywhere; a "No text field to paste into" notice with **Copy**; the text is in Activity.
- [ ] Press **Copy** on the notice, click into Notes and paste. Expected: the sentence.
- [ ] Focus a button or a list (for example a Finder sidebar item, or Tab to a button in a dialog) and dictate. Expected: the same notice, no paste.
- [ ] Focus a web page text box in the browser (for example the Gmail body) and dictate. Expected: it **pastes** (web text boxes must never be treated as "nowhere").
- [ ] Windows: try the same with the desktop focused and note what happens (the notice is macOS-only in P1; the text must still be in Activity).

## 8. Screen lock (induced failure)

The goal: no recording is ever lost when the screen locks mid-dictation.

- [ ] **Hands-free, then lock.** Start a hands-free dictation and speak for about 10 seconds. Lock the screen (macOS Ctrl+Cmd+Q; Windows Win+L). Wait 30 seconds, unlock. Stop the dictation if it's still running. Expected: either the text is pasted or Activity has a failed entry with its audio and **Retry**. Press Retry. Expected: the full sentence comes back.
- [ ] **Hold, then lock.** Hold the shortcut, speak, and lock the screen while still holding; release after locking. Unlock. Expected: the recording is not stuck (the bar isn't left showing "recording" forever); the audio is in Activity either as text or as a failed entry you can Retry.
- [ ] **Lock by timeout.** Set the screen to lock after 1 minute, start hands-free and let it lock by itself. Expected: as above.
- [ ] After each case, a normal dictation works without restarting the app.

Write down exactly what happened in each case (pasted / failed entry / stuck / missing): this test has not been run on any OS yet.

## 9. Other induced failures (spot checks)

These are covered by automated tests on Linux; repeat them once per OS.

- [ ] **Network drop.** Start a hands-free dictation, turn Wi-Fi off, speak, stop. Expected: nothing pasted; clipboard untouched; a "Couldn't reach the server" notice; a failed entry with Retry. Turn Wi-Fi on and press Retry. Expected: the text appears in Activity (not pasted).
- [ ] **Brief drop.** Start hands-free, turn Wi-Fi off for 3 seconds and back on, then stop. Expected: the text is pasted as normal.
- [ ] **App killed.** Start a hands-free dictation, speak, then force-quit Vocalis (Activity Monitor / Task Manager). Reopen. Expected: Activity shows a failed "Vocalis closed before this recording finished" entry with Retry, and Retry returns the text.

## 10. Microphone switching

- [ ] Pick the second microphone in the wizard's Microphone step. Dictate. Expected: the second mic is used (check by tapping or muting the built-in mic).
- [ ] Set the picker back to **System default**, then change the system's default input device in the OS sound settings. Dictate. Expected: the new system default is used without restarting Vocalis.
- [ ] Choose the USB/wired mic explicitly, quit Vocalis, unplug it, reopen and dictate. Expected: Vocalis falls back to the system default; the dictation works.
- [ ] Plug it back in. Expected: the next dictation uses it again.
- [ ] **Unplug mid-recording.** Start a hands-free dictation on the USB mic, speak, unplug it. Expected: the dictation stops without a crash, and the audio recorded so far is in Activity with Retry. Note the wording shown on the entry.
- [ ] Bluetooth (if available): pick the Bluetooth headset and dictate. Note the start delay and the quality compared with the wired mic.

## 11. Sign-off

| | macOS | Windows |
| --- | --- | --- |
| All boxes ticked, or failures filed | [ ] | [ ] |
| Issues filed (links) | | |
| Tester | | |

When both columns are signed off, record the result in `docs/PROGRESS.md` and tick the matching rows in `docs/PARITY.md`.
