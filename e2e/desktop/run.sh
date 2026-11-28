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
start_gateway() {
  SUPABASE_JWT_SECRET="$SECRET" GROQ_API_KEY=e2e GROQ_BASE_URL="http://127.0.0.1:$STT_PORT" PORT="$GW_PORT" \
    "${CARGO_TARGET_DIR:-target}/debug/vocalis-gateway" >>"$WORK/gateway.log" 2>&1 &
  GW_PID=$!; pids+=($GW_PID)
  for _ in $(seq 50); do curl -sf "http://127.0.0.1:$GW_PORT/healthz" >/dev/null && return; sleep 0.1; done
}
start_gateway

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
    VOCALIS_DEV_TOKEN="$TOKEN" VOCALIS_DEV_WAV="$WORK/speech.wav" VOCALIS_DEV_LOCK_FILE="$WORK/locked" RUST_LOG=info \
    "${CARGO_TARGET_DIR:-target}/debug/vocalis-desktop" >"$1" 2>&1 &
  DESKTOP_PID=$!; pids+=($DESKTOP_PID)
  # Cold starts after a rebuild can take a while: wait for the engine, then
  # give the webview and key listener a moment.
  for _ in $(seq 300); do grep -q "dictation engine ready" "$1" 2>/dev/null && break; sleep 0.1; done
  sleep 2
}
launch_desktop "$WORK/desktop.log"

"$PY" e2e/desktop/target_app.py "$WORK/other.txt" "E2E other" 200 nofocus & pids+=($!)
sleep 1
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

# Screen lock mid-recording (simulated with the dev lock file): the recording
# stops, nothing is sent or pasted, the speech is kept as a failed record, and
# the next dictation (below) still works, so no key is left "held".
xdotool keydown F8
sleep 1.2
touch "$WORK/locked"
sleep 1.5
xdotool keyup F8
rm -f "$WORK/locked"
sleep 2
after_lock="$(cat "$WORK/pasted.txt" 2>/dev/null || true)"
lock_logged="$(sed 's/\x1b\[[0-9;]*m//g' "$WORK/desktop.log" | grep -c 'recording interrupted by a screen lock or sleep' || true)"

# Paste into the starting app: start in the target, switch to the other
# window while speaking; the text must still land in the target.
xdotool search --name "E2E target" windowfocus --sync 2>/dev/null || true
sleep 0.5
xdotool keydown F8
sleep 1
xdotool search --name "E2E other" windowfocus --sync 2>/dev/null || true
sleep 1.5
xdotool keyup F8
sleep 4
refocused="$(cat "$WORK/pasted.txt" 2>/dev/null || true)"
other="$(cat "$WORK/other.txt" 2>/dev/null || true)"
clip="$(xclip -selection clipboard -o 2>/dev/null || true)"
recordings="$(find "$WORK/data" -name '*.wav.enc' | wc -l)"

# Network drop mid-recording, back before release: the recording is resent
# from the journal and still pasted.
xdotool search --name "E2E target" windowfocus --sync 2>/dev/null || true
sleep 0.5
xdotool keydown F8
sleep 1
kill -9 "$GW_PID"
sleep 0.5
start_gateway
sleep 0.5
xdotool keyup F8
sleep 4
after_drop="$(cat "$WORK/pasted.txt" 2>/dev/null || true)"

# Network down through release: nothing pasted, clipboard untouched, a failed
# record that keeps its audio for Retry.
kill -9 "$GW_PID"
xdotool keydown F8
sleep 2
xdotool keyup F8
sleep 4
after_offline="$(cat "$WORK/pasted.txt" 2>/dev/null || true)"
clip_offline="$(xclip -selection clipboard -o 2>/dev/null || true)"
recordings_offline="$(find "$WORK/data" -name '*.wav.enc' | wc -l)"
offline_failed="$(sed 's/\x1b\[[0-9;]*m//g' "$WORK/desktop.log" | grep -c 'dictation failed; audio kept for retry' || true)"
start_gateway

# Crash mid-recording: hold the chord, kill -9 the app while audio is being
# journaled, then restart it and look for the recovery report.
xdotool keydown F8
sleep 2
kill -9 "$DESKTOP_PID" 2>/dev/null || true
xdotool keyup F8
sleep 1
launch_desktop "$WORK/desktop2.log"
recovered="$(sed 's/\x1b\[[0-9;]*m//g' "$WORK/desktop2.log" | grep -c 'recovered interrupted recordings recovered=1' || true)"
echo "pasted:     '$pasted'"
echo "clipboard:  '$clip'"
echo "recordings: $recordings"
echo "after lock: '$after_lock' (interrupted $lock_logged)"
echo "refocused:  '$refocused'"
echo "other:      '$other'"
echo "after drop: '$after_drop'"
echo "offline:    failed=$offline_failed recordings=$recordings_offline clipboard='$clip_offline'"
echo "recovered:  $recovered"
echo "dictations: $(grep -c '"dictation processed"' "$WORK/gateway.log" || true)"
echo "stt calls:  $(wc -l < "$WORK/stt.log" 2>/dev/null || echo 0)"

status=0
[[ "$pasted" == "Hello from the end to end test" ]] || { echo "FAIL: unexpected pasted text"; status=1; }
[[ "$clip" == "ORIGINAL CLIPBOARD" ]] || { echo "FAIL: clipboard not restored"; status=1; }
[[ "$recordings" -ge 1 ]] || { echo "FAIL: no encrypted recording kept"; status=1; }
[[ "$(grep -c '"dictation processed"' "$WORK/gateway.log" || true)" -eq 3 ]] || { echo "FAIL: expected exactly three processed dictations (default chord must be inactive)"; status=1; }
[[ "$after_lock" == "$pasted" ]] || { echo "FAIL: something was pasted after the screen locked"; status=1; }
[[ "$lock_logged" -eq 1 ]] || { echo "FAIL: the screen lock did not interrupt the recording"; status=1; }
[[ "$after_drop" == "Hello from the end to end testHello from the end to end testHello from the end to end test" ]] || { echo "FAIL: dictation was lost when the connection dropped mid-recording"; status=1; }
[[ "$after_offline" == "$after_drop" ]] || { echo "FAIL: something was pasted while offline"; status=1; }
[[ "$clip_offline" == "ORIGINAL CLIPBOARD" ]] || { echo "FAIL: clipboard changed while offline"; status=1; }
[[ "$offline_failed" -eq 1 ]] || { echo "FAIL: the offline dictation did not become a failed record"; status=1; }
[[ "$recordings_offline" -eq 5 ]] || { echo "FAIL: expected 5 kept recordings after the offline dictation (incl. the locked one), got $recordings_offline"; status=1; }
[[ "$refocused" == "Hello from the end to end testHello from the end to end test" ]] || { echo "FAIL: second dictation did not go back to the starting app"; status=1; }
[[ -z "$other" ]] || { echo "FAIL: text was pasted into the window the user switched to"; status=1; }
[[ "$recovered" -eq 1 ]] || { echo "FAIL: audio from the killed recording was not recovered"; status=1; }
if grep -q "hello from the end to end test" "$WORK/gateway.log"; then echo "FAIL: transcript in gateway logs"; status=1; fi
if [[ $status -ne 0 ]]; then
  echo "--- desktop log"; tail -30 "$WORK/desktop.log"; echo "--- desktop log (restart)"; tail -15 "$WORK/desktop2.log"; echo "--- gateway log"; tail -10 "$WORK/gateway.log"
else
  echo "E2E PASSED"
fi
exit $status
