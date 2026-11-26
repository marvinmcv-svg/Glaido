#!/usr/bin/env bash
# Clean-room guard (ADR-0005, master prompt §9.7).
# Fails if the reference product's trademark or distinctive identity markers
# appear anywhere outside docs/ (where the audit and specs legitimately name it).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

# Built from parts so this file does not match itself.
patterns=(
  "gla""ido"
  "#bff""549"
  "aspe""kta"
)
regex="$(IFS='|'; echo "${patterns[*]}")"

hits="$(git ls-files -co --exclude-standard \
  | grep -v -E '^(docs/|scripts/no-reference-strings\.sh$)' \
  | grep -v -E '(^|/)(pnpm-lock\.yaml|Cargo\.lock)$' \
  | xargs -r grep -I -n -i -E "$regex" -- 2>/dev/null || true)"

if [[ -n "$hits" ]]; then
  echo "Clean-room check failed: reference identity strings found outside docs/:" >&2
  echo "$hits" >&2
  exit 1
fi
echo "Clean-room check passed."
