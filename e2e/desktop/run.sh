#!/usr/bin/env bash
# Desktop dictation E2E on Linux/X11 (master prompt §8 "Desktop E2E"):
# real desktop app + real gateway + synthetic hotkeys (xdotool) + a WAV in place
# of the microphone + a mock Groq-compatible STT server + a Tk text box target.
# Asserts: the cleaned text is pasted into the focused app, the user's
# clipboard is restored, and an encrypted recording was kept. Then kills the
# app mid-recording and checks the restarted app recovers that audio as a
# failed record (P1 exit: no lost audio when the app is killed).
#
# Needs: Xvfb, xdotool, xclip, python3 with tkinter, a built gateway and a
# desktop build with the frontend embedded:
#   cargo build -p vocalis-gateway
#   (cd apps/desktop && pnpm build) && cargo build -p vocalis-desktop --features tauri/custom-protocol
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
# The target app needs tkinter, which some Python builds lack: pick one that has it.
PY="${E2E_PYTHON:-}"
if [[ -z "$PY" ]]; then
  for c in python3 /usr/bin/python3 /usr/bin/python3.12 /usr/bin/python3.13 /usr/bin/python3.11; do
    if command -v "$c" >/dev/null && "$c" -c 'import tkinter' 2>/dev/null; then PY="$c"; break; fi
  done
fi
[[ -n "$PY" ]] || { echo "FAIL: no python3 with tkinter found (set E2E_PYTHON)"; exit 1; }
WORK="${E2E_WORK:-$(mktemp -d)}"
SECRET="e2e-secret-with-at-least-32-characters!!"
STT_PORT=18911; GW_PORT=18912
export DISPLAY="${DISPLAY_E2E:-:97}"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT

Xvfb "$DISPLAY" -screen 0 1280x800x24 >/dev/null 2>&1 & pids+=($!)
sleep 1
TOKEN="$("$PY" e2e/desktop/make_inputs.py "$WORK/speech.wav" "$SECRET")"
"$PY" e2e/desktop/mock_stt.py "$STT_PORT" "hello from the end to end test" "$WORK/stt.log" & pids+=($!)
SUPABASE_JWT_SECRET="$SECRET" GROQ_API_KEY=e2e GROQ_BASE_URL="http://127.0.0.1:$STT_PORT" PORT="$GW_PORT" \
  ./target/debug/vocalis-gateway >"$WORK/gateway.log" 2>&1 & pids+=($!)

printf 'ORIGINAL CLIPBOARD' | xclip -selection clipboard &
sleep 0.5

# Settings as the setup wizard leaves them, with a non-default shortcut, so the
# run also proves that saved hotkeys drive the engine.
mkdir -p "$WORK/data/app.vocalis.desktop"
cat >"$WORK/data/app.vocalis.desktop/settings.json" <<'JSON'
{"languages":["en"],"setupCompleted":true,
 "hotkeys":{"linux":{"dictation":"f8","dictationHandsfree":"shift+f8"}}}
JSON

launch_desktop() {
  HOME="$WORK/home" XDG_DATA_HOME="$WORK/data" VOCALIS_GATEWAY_URL="http://127.0.0.1:$GW_PORT" \
    VOCALIS_DEV_TOKEN="$TOKEN" VOCALIS_DEV_WAV="$WORK/speech.wav" RUST_LOG=info \
    ./target/debug/vocalis-desktop >"$1" 2>&1 &
  DESKTOP_PID=$!; pids+=($DESKTOP_PID)
}
launch_desktop "$WORK/desktop.log"
sleep 6

"$PY" e2e/desktop/target_app.py "$WORK/pasted.txt" & pids+=($!)
sleep 2
xdotool search --name "E2E target" windowactivate --sync 2>/dev/null || true

# The default chord (Left Ctrl + Space) must no longer start anything.
xdotool keydown Control_L keydown space
sleep 1
xdotool keyup space keyup Control_L
sleep 1
# Hold the configured shortcut (F8) for 2.5 s. (Not Right Ctrl: xdotool also
# presses Left Ctrl for it, which the tracker rightly treats as another chord.)
xdotool keydown F8
sleep 2.5
xdotool keyup F8
sleep 4

pasted="$(cat "$WORK/pasted.txt" 2>/dev/null || true)"
clip="$(xclip -selection clipboard -o 2>/dev/null || true)"
recordings="$(find "$WORK/data" -name '*.wav.enc' | wc -l)"

# Crash mid-recording: hold the chord, kill -9 the app while audio is being
# journaled, then restart it and look for the recovery report.
xdotool keydown F8
sleep 2
kill -9 "$DESKTOP_PID" 2>/dev/null || true
xdotool keyup F8
sleep 1
launch_desktop "$WORK/desktop2.log"
sleep 6
recovered="$(sed 's/\x1b\[[0-9;]*m//g' "$WORK/desktop2.log" | grep -c 'recovered interrupted recordings recovered=1' || true)"
echo "pasted:     '$pasted'"
echo "clipboard:  '$clip'"
echo "recordings: $recordings"
echo "recovered:  $recovered"
echo "dictations: $(grep -c '"dictation processed"' "$WORK/gateway.log" || true)"
echo "stt calls:  $(wc -l < "$WORK/stt.log" 2>/dev/null || echo 0)"

status=0
[[ "$pasted" == "Hello from the end to end test" ]] || { echo "FAIL: unexpected pasted text"; status=1; }
[[ "$clip" == "ORIGINAL CLIPBOARD" ]] || { echo "FAIL: clipboard not restored"; status=1; }
[[ "$recordings" -ge 1 ]] || { echo "FAIL: no encrypted recording kept"; status=1; }
[[ "$(grep -c '"dictation processed"' "$WORK/gateway.log" || true)" -eq 1 ]] || { echo "FAIL: expected exactly one dictation (default chord must be inactive)"; status=1; }
[[ "$recovered" -eq 1 ]] || { echo "FAIL: audio from the killed recording was not recovered"; status=1; }
if grep -q "hello from the end to end test" "$WORK/gateway.log"; then echo "FAIL: transcript in gateway logs"; status=1; fi
if [[ $status -ne 0 ]]; then
  echo "--- desktop log"; tail -30 "$WORK/desktop.log"; echo "--- desktop log (restart)"; tail -15 "$WORK/desktop2.log"; echo "--- gateway log"; tail -10 "$WORK/gateway.log"
else
  echo "E2E PASSED"
fi
exit $status
