#!/usr/bin/env bash
# Prepares the disposable test database: builds the migrate tool OFFLINE first
# (the schema does not exist yet, so live query verification of its core-crate
# deps would fail; a build failure leaves the database intact), drops and
# recreates the target database, then applies every extension schema.
#
#   scripts/test-db.sh               prepare it; progress on stderr
#   scripts/test-db.sh --print-url   print the URL it targets, change nothing
#
# The target is TEST_DATABASE_URL, default a `systemprompt_test` database on
# the local server. The default URL carries no password: psql and sqlx read it
# from PGPASSWORD or ~/.pgpass. Used by `just test-shard`, `just test-rust` and
# the test-workspace phases of scripts/gate.sh.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

db="${TEST_DATABASE_URL:-postgres://systemprompt_admin@localhost:5432/systemprompt_test}"
if [ "${1:-}" = "--print-url" ]; then
    printf '%s\n' "$db"
    exit 0
fi
[ "$#" -eq 0 ] || { echo "usage: $0 [--print-url]" >&2; exit 2; }

base="${db%/*}"
name="${db##*/}"
echo "▶ building migration tool (offline)" >&2
migrate_bin=$(SQLX_OFFLINE=true cargo build --manifest-path crates/tests/Cargo.toml \
    -p systemprompt-test-migrate --message-format=json-render-diagnostics \
    | jq -r 'select(.reason == "compiler-artifact" and .executable != null) | .executable' | tail -n1)
test -x "${migrate_bin}"
echo "▶ resetting test database: ${name}" >&2
psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${name}\" WITH (FORCE);" >/dev/null
psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"${name}\";" >/dev/null
echo "▶ applying extension schemas" >&2
SQLX_OFFLINE=true DATABASE_URL="${db}" "${migrate_bin}"
