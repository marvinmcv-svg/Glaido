# Spec 4.1: Dictation (core loop)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.1 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Lifecycle:**
1. Hotkey pressed: start sound plays and the dictation bar appears.
2. Audio is captured with noise reduction.
3. Audio streams to the gateway for STT.
4. On stop, the formatting LLM applies style, rules, dictionary and snippets.
5. The result is pasted into the focused field.
6. A record is saved to local History.

Short utterances return in about 1–3 s. All four processing steps finish after you stop speaking.

**Recording modes**

| Mode | Behavior |
|---|---|
| Push-to-talk (default) | Hold to record. Release to transcribe and paste. |
| Hands-free (toggle) | Press to start, press again to stop. **Esc always cancels** (nothing pasted, but the audio is kept). Optional **"Enter to stop and paste"** (off by default; when on, Enter stops and pastes in one step, for dictation and commands alike). |
| Double-tap variants | For single-key hotkeys: *"double-tap and hold"* for push-to-talk, and *"double-tap to start / double-tap to stop"* for hands-free. A single tap ends a hands-free session started by double-tap. The same key can hold-to-dictate **and** double-tap for hands-free. |

**Limits and resilience**
- **10-minute cap** per recording. Warn before the cutoff, then stop automatically and still transcribe everything.
- **Audio is preserved** if the app crashes, the session stalls, the mic disconnects, the screen locks, the network drops, processing times out, or the weekly quota is hit. Each case becomes a **failed record with a Retry button**.
- **Slow connection** state appears in the bar while audio catches up. Results that arrive after the user has moved on go to History instead of being pasted.
- **Automatic failover** to a backup route or region when the connection degrades.
- **Works behind corporate proxies** (honor the system proxy and PAC files).
- The local history store **checks and repairs itself** (integrity check plus recovery).

**Paste semantics**
- Paste via clipboard plus a synthetic paste keystroke, then **restore the user's previous clipboard** unless they've copied something new since. The **"Copy to clipboard"** setting (off by default) also leaves the result on the clipboard.
- **Paste into the starting app** (macOS and Windows): if the user switched apps mid-dictation, return focus to the original app and paste there.
- **Nowhere to paste** (macOS): no text field was focused when dictation ended. Show a notification with **Copy**. The text is always saved in History. (v0.5.2 made the copy offer optional.)
- **Could not paste** (Linux): show a notification with Copy.
- Terminals on Linux paste with **Ctrl+Shift+V**.

**Live feedback UI: the dictation bar**
- Bottom-center, a pill with a **live waveform**. It **never takes focus**, stays always on top, and also shows over full-screen games.
- Position: **Bottom / Raised / High**.
- Shows states: recording, processing, slow connection, nearing limit, error.
- **Interaction sounds** for start and stop (toggle).
- **Background audio while dictating:** *Mute speakers* / *Leave playing* / *Pause media and resume afterward*. The default is to mute on macOS and leave playing on Windows; on Windows the setting is labeled "Mute system audio".
- **Input level meter** in settings ("can it hear you?").

**Voice activation (wake word inside a dictation).** If `Vocalis` or "Hey `Vocalis`" is **within the first two words** of a dictation (case-insensitive), strip it and run the rest as a **Command** (§4.2). The dictation bar morphs into the command window, and History stores one command entry with no stray transcript. Saying only the wake word opens the command window with a "How can I help?" prompt and waits for the commands hotkey. Text selected before dictating is sent as context. This is always on, with no setting.

**Languages.** 100+ dictation languages, chosen by language rather than region (one "English" covers every accent). Selecting several turns on auto-detect; selecting one gives maximum accuracy. At least one must stay selected. The default comes from the **system locale**.

## Open questions / UNKNOWN

- Gesture details and Esc/raw/snippet ordering settled in ADR-0010 (`crates/core/src/gesture.rs`, `formatting.rs`).
- Wake-word tokenization settled in ADR-0010 item 5 (`crates/core/src/wake.rs`).
