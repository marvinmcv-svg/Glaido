#!/usr/bin/env bash
# Generates OUT/<id>.wav (16 kHz, mono, PCM16) for every line of the latency
# corpus (utterances.tsv) with a local text-to-speech engine.
#
# Engine order (first that works wins):
#   1. Piper TTS (pip-installed into a venv next to the output dir, one English voice)
#   2. macOS `say` + `afconvert`
#   3. `espeak-ng`, then `espeak`
# Resampling to 16 kHz uses ffmpeg, then sox, then a pure-Python fallback.
#
# Usage: e2e/latency/make_speech.sh [--force] [--out DIR] [--manifest FILE] [--engine NAME]
#   --force          regenerate files that already exist
#   --out DIR        output directory (default: e2e/latency/audio)
#   --manifest FILE  corpus file (default: e2e/latency/utterances.tsv)
#   --engine NAME    piper | say | espeak (skip auto-detection)
# Environment: PIPER_VOICE (default en_US-lessac-medium), PIPER_LENGTH_SCALE (1.12),
#   PIPER_SENTENCE_SILENCE (0.3 s), PYTHON (default python3).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$here/audio"
manifest="$here/utterances.tsv"
force=0
engine=""
voice="${PIPER_VOICE:-en_US-lessac-medium}"
# Slightly slower than the voice's default, with a short pause between
# sentences, so it sounds like dictation and a long line lands near 60 s.
piper_length_scale="${PIPER_LENGTH_SCALE:-1.12}"
piper_sentence_silence="${PIPER_SENTENCE_SILENCE:-0.3}"
PY="${PYTHON:-python3}"

die() {
  echo "make_speech: error: $*" >&2
  exit 1
}
log() { echo "make_speech: $*" >&2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --force) force=1 ;;
    --out)
      out="${2:?--out needs a directory}"
      shift
      ;;
    --manifest)
      manifest="${2:?--manifest needs a file}"
      shift
      ;;
    --engine)
      engine="${2:?--engine needs a name}"
      shift
      ;;
    -h | --help)
      sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) die "unknown argument: $1 (try --help)" ;;
  esac
  shift
done

[[ -f "$manifest" ]] || die "manifest not found: $manifest"
command -v "$PY" > /dev/null || die "python3 is required (set PYTHON=...)"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
piper_home="$(dirname "$out")/.piper"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# ---------------------------------------------------------------- resampling
# to16k IN OUT: convert any WAV/AIFF the tools understand to 16 kHz mono PCM16.
to16k() {
  if command -v ffmpeg > /dev/null; then
    ffmpeg -nostdin -loglevel error -y -i "$1" -ar 16000 -ac 1 -c:a pcm_s16le "$2"
  elif command -v sox > /dev/null; then
    sox "$1" -r 16000 -c 1 -b 16 -e signed-integer "$2"
  else
    "$PY" - "$1" "$2" << 'PYEOF'
# Pure-stdlib fallback: PCM16 WAV in, 16 kHz mono PCM16 WAV out.
# Downmixes, low-passes with a short moving average when downsampling, then
# resamples by linear interpolation. Good enough for STT fixtures.
import array, sys, wave
src, dst = sys.argv[1], sys.argv[2]
with wave.open(src, "rb") as w:
    ch, width, rate, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
    if width != 2:
        sys.exit(f"pure-python resampler needs 16-bit input, got {8 * width}-bit; install ffmpeg or sox")
    raw = array.array("h", w.readframes(n))
if sys.byteorder == "big":
    raw.byteswap()
mono = [sum(raw[i:i + ch]) / ch for i in range(0, len(raw), ch)] if ch > 1 else list(raw)
target = 16000
if rate != target:
    if rate > target:
        k = max(1, round(rate / target))
        if k > 1:
            acc, smoothed = 0.0, []
            for i, v in enumerate(mono):
                acc += v
                if i >= k:
                    acc -= mono[i - k]
                smoothed.append(acc / min(i + 1, k))
            mono = smoothed
    step = rate / target
    count = int(len(mono) / step)
    res = []
    for j in range(count):
        pos = j * step
        i = int(pos)
        frac = pos - i
        a = mono[i]
        b = mono[i + 1] if i + 1 < len(mono) else a
        res.append(a + (b - a) * frac)
    mono = res
pcm = array.array("h", (max(-32768, min(32767, int(round(v)))) for v in mono))
if sys.byteorder == "big":
    pcm.byteswap()
with wave.open(dst, "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(target)
    w.writeframes(pcm.tobytes())
PYEOF
  fi
}

# ------------------------------------------------------------------- engines
# Each synth_<engine> TEXTFILE OUTWAV writes a 16 kHz mono PCM16 WAV.

piper_bin=""
piper_model=""

setup_piper() {
  local venv="$piper_home/venv" vdir="$piper_home/voices"
  mkdir -p "$vdir"
  if [[ ! -x "$venv/bin/piper" ]]; then
    log "installing Piper TTS into $venv ..."
    "$PY" -m venv "$venv" > "$tmp/venv.log" 2>&1 || {
      log "python venv creation failed (see below)"
      tail -n 5 "$tmp/venv.log" >&2
      return 1
    }
    "$venv/bin/pip" install --quiet --disable-pip-version-check piper-tts > "$tmp/pip.log" 2>&1 || {
      log "pip install piper-tts failed (see below)"
      tail -n 5 "$tmp/pip.log" >&2
      return 1
    }
  fi
  [[ -x "$venv/bin/piper" ]] || {
    log "piper-tts installed but no piper executable found"
    return 1
  }
  piper_model="$vdir/$voice.onnx"
  if [[ ! -s "$piper_model" || ! -s "$piper_model.json" ]]; then
    # Voice names look like <lang>_<REGION>-<speaker>-<quality>.
    local lang_region="${voice%%-*}" rest="${voice#*-}"
    local speaker="${rest%-*}" quality="${rest##*-}" lang="${voice%%_*}"
    local base="https://huggingface.co/rhasspy/piper-voices/resolve/main/$lang/$lang_region/$speaker/$quality/$voice"
    log "downloading Piper voice $voice ..."
    local f
    for f in onnx onnx.json; do
      if ! curl -fsSL --retry 3 -o "$vdir/$voice.$f.part" "$base.$f?download=true"; then
        rm -f "$vdir/$voice.$f.part"
        log "could not download $base.$f"
        return 1
      fi
      mv "$vdir/$voice.$f.part" "$vdir/$voice.$f"
    done
  fi
  piper_bin="$venv/bin/piper"
  # Smoke test: the install can succeed and still fail at run time (onnxruntime, CPU).
  echo "Testing one two three." > "$tmp/smoke.txt"
  synth_piper "$tmp/smoke.txt" "$tmp/smoke.wav" > "$tmp/smoke.log" 2>&1 || {
    log "Piper smoke test failed (see below)"
    tail -n 5 "$tmp/smoke.log" >&2
    return 1
  }
}

synth_piper() {
  "$piper_bin" --model "$piper_model" --output_file "$tmp/raw.wav" \
    --length_scale "$piper_length_scale" --sentence_silence "$piper_sentence_silence" \
    < "$1" > /dev/null 2>> "$tmp/piper.log"
  to16k "$tmp/raw.wav" "$2"
}

setup_say() {
  [[ "$(uname -s)" == Darwin ]] && command -v say > /dev/null && command -v afconvert > /dev/null
}

synth_say() {
  say -f "$1" -o "$tmp/raw.aiff"
  afconvert -f WAVE -d LEI16@16000 -c 1 "$tmp/raw.aiff" "$2"
}

espeak_bin=""
setup_espeak() {
  espeak_bin="$(command -v espeak-ng || command -v espeak || true)"
  [[ -n "$espeak_bin" ]]
}

synth_espeak() {
  "$espeak_bin" -v en-us -s 165 -f "$1" -w "$tmp/raw.wav"
  to16k "$tmp/raw.wav" "$2"
}

# ---------------------------------------------------------- engine selection
if [[ -n "$engine" ]]; then
  case "$engine" in
    piper | say | espeak) "setup_$engine" || die "requested engine '$engine' is not available" ;;
    *) die "unknown engine '$engine' (piper | say | espeak)" ;;
  esac
else
  for candidate in piper say espeak; do
    if "setup_$candidate"; then
      engine="$candidate"
      break
    fi
    log "engine '$candidate' unavailable, trying the next one"
  done
  [[ -n "$engine" ]] || die "no TTS engine available. Install one of: Piper (needs python3-venv and network), macOS say, espeak-ng."
fi

case "$engine" in
  piper) label="piper ($voice)" ;;
  say) label="macOS say + afconvert" ;;
  espeak) label="$(basename "$espeak_bin")" ;;
esac
resampler="python stdlib"
command -v sox > /dev/null && resampler="sox"
command -v ffmpeg > /dev/null && resampler="ffmpeg"
[[ "$engine" == say ]] && resampler="afconvert"
echo "Engine: $label; resampler: $resampler; output: $out"

# ------------------------------------------------------------------ generate
made=0
skipped=0
while IFS=$'\t' read -r id kind text || [[ -n "${id:-}" ]]; do
  [[ -z "$id" || "$id" == \#* ]] && continue
  [[ -n "${text:-}" ]] || die "line for '$id' has no text (expected id<TAB>kind<TAB>text)"
  dest="$out/$id.wav"
  if [[ -s "$dest" && "$force" -eq 0 ]]; then
    skipped=$((skipped + 1))
    continue
  fi
  printf '%s\n' "$text" > "$tmp/text.txt"
  "synth_$engine" "$tmp/text.txt" "$tmp/out.wav" || die "synthesis failed for $id ($kind)"
  mv "$tmp/out.wav" "$dest"
  made=$((made + 1))
done < "$manifest"

echo "Generated $made file(s), skipped $skipped existing (use --force to regenerate)."

# ------------------------------------------------------------------ summary
"$PY" - "$manifest" "$out" << 'PYEOF'
import os, sys, wave
manifest, out = sys.argv[1], sys.argv[2]
stats = {}
missing = []
for line in open(manifest, encoding="utf-8"):
    line = line.rstrip("\n")
    if not line or line.startswith("#"):
        continue
    uid, kind, _ = line.split("\t", 2)
    path = os.path.join(out, uid + ".wav")
    if not os.path.exists(path):
        missing.append(uid)
        continue
    with wave.open(path, "rb") as w:
        assert (w.getframerate(), w.getnchannels(), w.getsampwidth()) == (16000, 1, 2), path
        secs = w.getnframes() / 16000
    stats.setdefault(kind, []).append(secs)
for kind, d in sorted(stats.items()):
    print(f"  {kind:5}: {len(d):2} files, total {sum(d):7.1f} s, min {min(d):5.1f} s, max {max(d):5.1f} s")
print(f"  all  : {sum(len(d) for d in stats.values()):2} files, total {sum(sum(d) for d in stats.values()):7.1f} s")
if missing:
    sys.exit("missing: " + ", ".join(missing))
PYEOF
