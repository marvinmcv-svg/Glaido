#!/usr/bin/env bash
# Apply migrations to a throwaway Postgres and run the RLS tests.
# Usage: DATABASE_URL=postgres://... supabase/tests/run.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
url="${DATABASE_URL:?set DATABASE_URL to an empty test database}"
psql "$url" -v ON_ERROR_STOP=1 -q -f "$here/auth_shim.sql"
for f in "$here"/../migrations/*.sql; do psql "$url" -v ON_ERROR_STOP=1 -q -f "$f"; done
psql "$url" -v ON_ERROR_STOP=1 -q -f "$here/rls_test.sql"
