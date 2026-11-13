#!/usr/bin/env bash
# Prepares the disposable test database: builds the migrate tool first (a build
# failure leaves the database intact), drops and recreates the target database,
# then applies every extension schema.
#
#   scripts/test-db.sh               build the tool OFFLINE (committed .sqlx
#                                    caches), then reset and migrate
#   scripts/test-db.sh --online      build the tool ONLINE against the target
#                                    database as it stands before the reset, so
#                                    a query whose cache entry is not written
#                                    yet still compiles; the database must exist
#   scripts/test-db.sh --print-url   print the URL it targets, change nothing
#
# The target is TEST_DATABASE_URL, default a `systemprompt_test` database on
# the local server. The default URL carries no password: psql and sqlx read it
# from PGPASSWORD or ~/.pgpass. Used by `just test-shard`, `just test-rust`, the
# test-workspace phases of scripts/gate.sh and scripts/regen.sh.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

db="${TEST_DATABASE_URL:-postgres://systemprompt_admin@localhost:5432/systemprompt_test}"
build_env=(SQLX_OFFLINE=true)
case "${1:-}" in
    --print-url) printf '%s\n' "$db"; exit 0 ;;
    --online) build_env=(SQLX_OFFLINE=false "DATABASE_URL=${db}"); shift ;;
    "") ;;
    *) echo "usage: $0 [--online|--print-url]" >&2; exit 2 ;;
esac
[ "$#" -eq 0 ] || { echo "usage: $0 [--online|--print-url]" >&2; exit 2; }

base="${db%/*}"
name="${db##*/}"
echo "▶ building migration tool (${build_env[0]})" >&2
migrate_bin=$(env "${build_env[@]}" cargo build --manifest-path crates/tests/Cargo.toml \
    -p systemprompt-test-migrate --message-format=json-render-diagnostics \
    | jq -r 'select(.reason == "compiler-artifact" and .executable != null) | .executable' | tail -n1)
test -x "${migrate_bin}"
echo "▶ resetting test database: ${name}" >&2
psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${name}\" WITH (FORCE);" >/dev/null
psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"${name}\";" >/dev/null
echo "▶ applying extension schemas" >&2
SQLX_OFFLINE=true DATABASE_URL="${db}" "${migrate_bin}"
