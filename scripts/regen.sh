#!/usr/bin/env bash
# Regenerates everything derived from the source tree that is committed, in the
# one order where a stale derivative cannot block the next step:
#
#   1. migrate   reset and migrate the target database (scripts/test-db.sh).
#                The migrate tool is built ONLINE against the database as it
#                stands before the reset when it exists, so a query added since
#                the caches were written still compiles; OFFLINE otherwise.
#   2. sqlx      the root cache and every published crate's own .sqlx
#                (scripts/sqlx-prepare.sh workspace, publish), then the audit
#                (scripts/sqlx-audit-caches.sh). sqlx-prepare.sh snapshots every
#                cache and restores it on failure, so no run leaves a pruned
#                cache behind; a shrinking cache fails unless --allow-prune.
#   3. hakari    crates/tests/common/hack and each test crate's line on it.
#   4. lockfiles `cargo metadata` over every workspace the justfile's
#                `workspaces` variable lists, so each Cargo.lock is current.
#   5. proof     `just build-offline` against the regenerated caches, unless
#                --no-proof.
#
#   scripts/regen.sh [--allow-prune] [--no-proof] [--dry-run]
#
# --dry-run prints the plan and runs nothing. The target database is the one
# `scripts/test-db.sh --print-url` names (TEST_DATABASE_URL).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

allow_prune=0
proof=1
dry_run=0
for arg in "$@"; do
    case "$arg" in
        --allow-prune) allow_prune=1 ;;
        --no-proof) proof=0 ;;
        --dry-run) dry_run=1 ;;
        *) echo "usage: $0 [--allow-prune] [--no-proof] [--dry-run]" >&2; exit 2 ;;
    esac
done

workspaces=$(sed -nE 's/^workspaces[[:space:]]*:=[[:space:]]*"([^"]*)".*/\1/p' justfile)
[ -n "$workspaces" ] || { echo "regen: no \`workspaces\` variable in the justfile" >&2; exit 2; }
db=$(bash scripts/test-db.sh --print-url)
prune_env=()
[ "$allow_prune" -eq 1 ] && prune_env=(PREPARE_ALLOW_PRUNE=1)

database_exists() {
    local name="${db##*/}"
    [ "$(psql "${db%/*}/postgres" -tAc "SELECT 1 FROM pg_database WHERE datname = '${name}'" 2>/dev/null)" = "1" ]
}

plan=()
plan+=("migrate|bash scripts/test-db.sh --online (offline when ${db##*/} does not exist yet)")
plan+=("sqlx|env DATABASE_URL=${db} ${prune_env[*]} bash scripts/sqlx-prepare.sh workspace")
plan+=("sqlx|env DATABASE_URL=${db} ${prune_env[*]} bash scripts/sqlx-prepare.sh publish")
plan+=("sqlx|bash scripts/sqlx-audit-caches.sh")
plan+=("hakari|cd crates/tests && cargo hakari generate && cargo hakari manage-deps --yes")
for ws in $workspaces; do
    plan+=("lockfiles|cargo metadata --format-version 1 --manifest-path ${ws}/Cargo.toml")
done
[ "$proof" -eq 1 ] && plan+=("proof|just build-offline")

if [ "$dry_run" -eq 1 ]; then
    for entry in "${plan[@]}"; do
        printf '%-10s %s\n' "${entry%%|*}" "${entry#*|}"
    done
    exit 0
fi

step() { echo "▶ regen: $*" >&2; }

if database_exists; then
    step "migrate (migrate tool built online against ${db##*/})"
    bash scripts/test-db.sh --online
else
    step "migrate (${db##*/} does not exist: migrate tool built offline)"
    bash scripts/test-db.sh
fi

step "sqlx caches"
env DATABASE_URL="$db" "${prune_env[@]}" bash scripts/sqlx-prepare.sh workspace
env DATABASE_URL="$db" "${prune_env[@]}" bash scripts/sqlx-prepare.sh publish
bash scripts/sqlx-audit-caches.sh

step "hakari"
(cd crates/tests && cargo hakari generate && cargo hakari manage-deps --yes)

step "lockfiles"
for ws in $workspaces; do
    cargo metadata --format-version 1 --manifest-path "${ws}/Cargo.toml" >/dev/null
done

if [ "$proof" -eq 1 ]; then
    step "proof: just build-offline"
    just build-offline
fi
echo "regen: done; review and commit the regenerated files (git status)" >&2
