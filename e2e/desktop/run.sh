#!/usr/bin/env bash
# Desktop dictation E2E on Linux/X11 (master prompt §8 "Desktop E2E"):
# real desktop app + real gateway + synthetic hotkeys (xdotool) + a WAV in place
# of the microphone + a mock Groq-compatible STT server + a Tk text box target.
# Asserts: the cleaned text is pasted into the focused app, the user's
# clipboard is restored, and an encrypted recording was kept.
#
# Needs: Xvfb, xdotool, xclip, python3 with tkinter, a built gateway and a
# desktop build with the frontend embedded:
#   cargo build -p vocalis-gateway
#   (cd apps/desktop && pnpm build) && cargo build -p vocalis-desktop --features tauri/custom-protocol
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
PY="${E2E_PYTHON:-python3}"
WORK="$(mktemp -d)"
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

HOME="$WORK/home" XDG_DATA_HOME="$WORK/data" VOCALIS_GATEWAY_URL="http://127.0.0.1:$GW_PORT" \
  VOCALIS_DEV_TOKEN="$TOKEN" VOCALIS_DEV_WAV="$WORK/speech.wav" RUST_LOG=info \
  ./target/debug/vocalis-desktop >"$WORK/desktop.log" 2>&1 & pids+=($!)
sleep 6

"$PY" e2e/desktop/target_app.py "$WORK/pasted.txt" & pids+=($!)
sleep 2
xdotool search --name "E2E target" windowactivate --sync 2>/dev/null || true

# Hold the Linux default dictation chord (Left Ctrl + Space) for 2.5 s.
xdotool keydown ctrl keydown space
sleep 2.5
xdotool keyup space keyup ctrl
sleep 4

pasted="$(cat "$WORK/pasted.txt" 2>/dev/null || true)"
clip="$(xclip -selection clipboard -o 2>/dev/null || true)"
recordings="$(find "$WORK/data" -name '*.wav.enc' | wc -l)"
echo "pasted:     '$pasted'"
echo "clipboard:  '$clip'"
echo "recordings: $recordings"
echo "stt calls:  $(wc -l < "$WORK/stt.log" 2>/dev/null || echo 0)"

status=0
[[ "$pasted" == "Hello from the end to end test" ]] || { echo "FAIL: unexpected pasted text"; status=1; }
[[ "$clip" == "ORIGINAL CLIPBOARD" ]] || { echo "FAIL: clipboard not restored"; status=1; }
[[ "$recordings" -ge 1 ]] || { echo "FAIL: no encrypted recording kept"; status=1; }
if grep -q "hello from the end to end test" "$WORK/gateway.log"; then echo "FAIL: transcript in gateway logs"; status=1; fi
if [[ $status -ne 0 ]]; then
  echo "--- desktop log"; tail -30 "$WORK/desktop.log"; echo "--- gateway log"; tail -10 "$WORK/gateway.log"
else
  echo "E2E PASSED"
fi
exit $status
