#!/usr/bin/env bash
# Fail when two tracked files would collide on a case-insensitive disk
# (Windows, default macOS): either the same path, or the same module name
# once the .ts/.tsx/.js extension is dropped (`Foo.tsx` vs `foo.ts`), which
# TypeScript treats as one import.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
clashes=$(
  {
    git ls-files
    git ls-files | grep -E '\.(tsx?|jsx?|mjs|cjs)$' | sed -E 's/\.(tsx?|jsx?|mjs|cjs)$//' | sort -u
  } | awk '{ k = tolower($0); if (seen[k] && seen[k] != $0) print seen[k] " <-> " $0; else seen[k] = $0 }'
)
if [ -n "$clashes" ]; then
  echo "These paths differ only in letter case and break on Windows/macOS:" >&2
  echo "$clashes" >&2
  exit 1
fi
echo "No case-only path clashes."
