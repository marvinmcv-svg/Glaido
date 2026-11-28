# Open-source analogs (R3)

Techniques worth learning from. **Verify each license on the repo before adopting anything**, and attribute in `THIRD_PARTY_NOTICES.md` if code is reused. GPL-licensed projects are reference-reading only: we do not copy their code into our (proprietary) app.

| Project | Likely license (verify) | What to learn | Adopt? |
|---|---|---|---|
| Handy (Tauri dictation app) | MIT | Tauri 2 structure for an always-on dictation app, push-to-talk via global shortcuts, cpal capture, overlay window | Patterns only |
| OpenWhispr | MIT | Multi-provider STT switching, local whisper fallback, paste strategies per OS | Patterns only |
| VoiceInk | GPL-3.0 | macOS AX-based context capture and paste | Read only (GPL) |
| whisper.cpp | MIT | Local STT (★ offline, P11), VAD, model quantization | Yes, as a dependency via bindings |
| tauri-plugin-global-shortcut | MIT / Apache-2.0 | Cross-platform shortcut registration (not enough for Fn or hold detection) | Yes for Spotlight/Settings hotkeys |
| rdev / evdev crates | MIT | Low-level key event capture, evdev on Linux | Evaluate in P1/P5 |
| arboard | MIT / Apache-2.0 | Clipboard save/restore | Yes |
| enigo | MIT | Synthetic keystrokes | Evaluate vs. native CGEvent/SendInput |
| cpal, rubato, webrtc-vad / silero | Apache-2.0 / MIT | Capture, resampling, VAD | Yes |
