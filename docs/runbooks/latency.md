# Latency harness

`vocalis-latency` (`tools/latency`) plays scripted dictations into a gateway through the same WebSocket client the desktop app uses (`crates/client`), and checks **stop → final result** against the §5.9 budgets:

| Check | Budget |
|---|---|
| Short dictation (≤ 15 words), p50 | < 1.0 s |
| Short dictation, p95 | < 2.0 s |
| 60-second dictation, p50 | < 3.0 s |

It also reports connect + start → ready time and, with recorded speech, the word error rate against the script. One warm-up dictation runs first and isn't counted. Dictations run one at a time.

**What it does not measure:** the local paste (clipboard + synthetic keystroke, a few milliseconds; the clipboard restore runs afterwards). The desktop E2E (`e2e/desktop/run.sh`) covers pasting.

## Inputs

- **Manifest** `e2e/latency/utterances.tsv`: one `id<TAB>kind<TAB>text` per line, `kind` is `short` (at most 15 words) or `long` (about 150 words ≈ 60 s). `#` starts a comment.
- **Audio**, one of:
  - `--audio-dir DIR` with `DIR/<id>.wav` per line: recorded speech, 16 kHz mono 16-bit PCM (anything else is rejected). Use this with real providers.
  - `--synthetic`: speech-like tones sized from the word count (2.5 words/s). Only meaningful with a mock provider.

## Validate the harness locally (mock provider)

```bash
cargo build -p vocalis-gateway -p vocalis-latency
```

```bash
e2e/latency/run_mock.sh
```

```bash
MOCK_STT_DELAY_MS=1500 e2e/latency/run_mock.sh
```

The first should print `GATE PASSED`; the second simulates a slow provider and should print `GATE FAILED` and exit 1.

## Run against a real gateway

1. The gateway needs real provider keys (`GROQ_API_KEY`, optionally `DEEPGRAM_API_KEY` and `ANTHROPIC_API_KEY`; see `docs/runbooks/deploy.md`). Check `GET /v1/meta`: no provider should report "unconfigured".
2. Get an access token for a test account. For example, sign in on the web app and copy `access_token` from the Supabase session, or use the desktop app's session. Tokens expire after about an hour.
3. Record or generate the speech for every manifest line into a folder as `<id>.wav` (16 kHz mono).
4. Run it from a machine near your users (latency depends on the route to the gateway):

```bash
cargo run --release -p vocalis-latency -- \
  --gateway https://gateway-production-80039d71.up.railway.app \
  --token "$VOCALIS_TOKEN" \
  --audio-dir path/to/wavs \
  --json latency-report.json --gate
```

Streaming runs in real time by default (`--speed 1`), as in the app. A full 50-line run with five 60-second dictations takes about eight minutes. `--limit N` runs only the first N lines.

Each dictation counts against the test account's weekly words, and the providers bill for the audio.
