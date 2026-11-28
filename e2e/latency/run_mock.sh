#!/usr/bin/env bash
# Validate the latency harness against a local gateway and a mock speech
# provider with a simulated delay (MOCK_STT_DELAY_MS, default 300 ms).
# Real-provider runs: docs/runbooks/latency.md.
#
#   cargo build -p vocalis-gateway -p vocalis-latency
#   e2e/latency/run_mock.sh                          # expect GATE PASSED
#   MOCK_STT_DELAY_MS=1500 e2e/latency/run_mock.sh   # expect GATE FAILED
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN="${CARGO_TARGET_DIR:-target}/debug"
SECRET="latency-secret-with-at-least-32-characters!!"
STT_PORT="${STT_PORT:-18921}"; GW_PORT="${GW_PORT:-18922}"
MANIFEST="${MANIFEST:-e2e/latency/utterances.tsv}"
[[ -f "$MANIFEST" ]] || MANIFEST=e2e/latency/sample.tsv
WORK="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT

MOCK_STT_DELAY_MS="${MOCK_STT_DELAY_MS:-300}" python3 e2e/desktop/mock_stt.py "$STT_PORT" \
  "can we move the design review to thursday afternoon" "$WORK/stt.log" & pids+=($!)
SUPABASE_JWT_SECRET="$SECRET" GROQ_API_KEY=mock GROQ_BASE_URL="http://127.0.0.1:$STT_PORT" PORT="$GW_PORT" \
  "$BIN/vocalis-gateway" >"$WORK/gateway.log" 2>&1 & pids+=($!)
for _ in $(seq 50); do curl -sf "http://127.0.0.1:$GW_PORT/healthz" >/dev/null && break; sleep 0.1; done

echo "mock provider delay: ${MOCK_STT_DELAY_MS:-300} ms; manifest: $MANIFEST"
# Synthetic audio streamed 20x faster than real time: stop -> final doesn't
# depend on the streaming pace, and a 60 s dictation then takes 3 s to send.
"$BIN/vocalis-latency" --gateway "http://127.0.0.1:$GW_PORT" --jwt-secret "$SECRET" \
  --manifest "$MANIFEST" --synthetic --speed "${SPEED:-20}" --json "$WORK/report.json" --gate "$@"
