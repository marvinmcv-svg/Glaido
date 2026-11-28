# Latency corpus

This folder holds the scripted utterances for the P1 latency exit check
(`docs/MASTER_PROMPT.md` §5.9 and §7): "50 scripted utterances pass the latency
budget". It also has a script that turns them into speech WAVs that a harness
can play into the dictation pipeline.

| Kind    | Count | Size                            | Budget (stop → text pasted) |
| ------- | ----- | ------------------------------- | --------------------------- |
| `short` | 45    | one sentence, at most 15 words  | p50 < 1.0 s, p95 < 2.0 s    |
| `long`  | 5     | about 150–170 words, about 60 s | p50 < 3.0 s                 |

All text is original. It is written the way people dictate: chat messages,
emails, code comments and review notes, meeting notes, reminders, a description
of terminal commands, money amounts, times, dates, version numbers,
punctuation-heavy lines and names from several languages (Oluwaseun, Joaquín,
Siobhán, Mei-Lin, François, Okonkwo and others). Long lines are spread through
the file (`u10`, `u20`, `u30`, `u40`, `u50`), so a run that stops early still
covers both kinds.

## Manifest format: `utterances.tsv`

- UTF-8 text, one utterance per line: `id<TAB>kind<TAB>text`.
- `id` is `u01` to `u50`, and it is also the audio file name (`u07.wav`).
- `kind` is `short` or `long`.
- `text` is the exact reference text. It never contains a tab, so split on the
  first two tabs only.
- Lines that start with `#` are comments, and blank lines are skipped.

The text is the reference for checking a transcript, but the latency budget
does not depend on the transcript being word-perfect. TTS audio fed to a real
STT service comes back with small differences (for example "9:30 a.m." or
"$1,284.50"), so compare loosely, for example with a word error rate threshold.

## Generating the audio

```sh
e2e/latency/make_speech.sh            # writes e2e/latency/audio/u01.wav ... u50.wav
e2e/latency/make_speech.sh --force    # regenerate everything
e2e/latency/make_speech.sh --out /tmp/vocalis-audio --engine espeak
```

Output is 16 kHz, mono, 16-bit PCM WAV, the format the desktop app's dev WAV
input expects. Files that already exist are skipped unless you pass `--force`.
At the end the script prints which engine it used and the file count and total
duration for each kind. It exits with an error if any file is missing.

The script uses the first engine that works:

1. **Piper TTS.** It runs `pip install piper-tts` into
   `<parent of the output dir>/.piper/venv` and downloads one English voice
   (`en_US-lessac-medium`, about 60 MB, from the `rhasspy/piper-voices`
   repository on Hugging Face) into `.piper/voices`. Then it runs a short test
   sentence. If any step fails, it prints the reason and tries the next
   engine. This needs `python3` with `venv` and network access the first time,
   and about 250 MB of disk.
2. **macOS `say` + `afconvert`.**
3. **`espeak-ng`, then `espeak`.** These sound robotic, but they are fine for
   timing.

Audio is converted to 16 kHz with ffmpeg if installed, then sox, and otherwise
with a small pure-Python resampler (16-bit input only).

Settings, from environment variables:

| Variable                 | Default               | Meaning                                                  |
| ------------------------ | --------------------- | -------------------------------------------------------- |
| `PIPER_VOICE`            | `en_US-lessac-medium` | Piper voice name (`<lang>_<REGION>-<speaker>-<quality>`) |
| `PIPER_LENGTH_SCALE`     | `1.12`                | Speaking rate; above 1 is slower                         |
| `PIPER_SENTENCE_SILENCE` | `0.3`                 | Seconds of pause after each sentence                     |
| `PYTHON`                 | `python3`             | Interpreter used for the venv and helpers                |

With the defaults and Piper, short clips run 2.7–5.9 s and long clips run
51–56 s, for about 440 s in total. The generated audio and the `.piper/`
folder are git-ignored. Do not commit them: anyone can regenerate them with
the script.
