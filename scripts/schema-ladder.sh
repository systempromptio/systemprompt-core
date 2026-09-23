#!/usr/bin/env bash
# Prove a released database upgrades to this tree's schema.
#
#   scripts/schema-ladder.sh <rung-tag> <current-migrator>
#
# Run from the checkout under test, which needs the rung's tag fetched.
#
# Builds `systemprompt-test-migrate` at <rung-tag> and installs it into a fresh
# database — the state every instance on that release is in. Then the current
# tree's migrator (<current-migrator>, already built) upgrades that database
# twice, a deploy and a restart, and installs a second database from scratch.
# The two catalogs (scripts/schema-catalog.sql) must be identical.
#
# Fresh-install gates cannot see a migration that only misbehaves against an
# established database; this is the one that can. `infra/events` 007 in 0.59.0
# dropped the only `actor_id` check on every upgrading instance and passed
# every other gate.
#
# Needs DATABASE_URL pointing at a server the caller may create databases on.
set -euo pipefail

RUNG="${1:?usage: schema-ladder.sh <rung-tag> <current-migrator>}"
CURRENT="$(realpath "${2:?usage: schema-ladder.sh <rung-tag> <current-migrator>}")"
: "${DATABASE_URL:?DATABASE_URL must point at a Postgres server}"
# Why: the tree under test is the checkout this runs in; the scripts come from
# beside this file, which a dispatched run takes from a newer commit than an
# old tag under test carries.
ROOT="$(git rev-parse --show-toplevel)"
SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
CATALOG="$SCRIPTS/schema-catalog.sql"
SERVER="${DATABASE_URL%/*}"
TAG_SLUG="$(printf '%s' "$RUNG" | tr -c 'a-zA-Z0-9' '_')"
UP="ladder_up_${TAG_SLUG}"
FRESH="ladder_fresh_${TAG_SLUG}"
WORK="$(mktemp -d)"

cleanup() {
    git -C "$ROOT" worktree remove --force "$WORK/rung" >/dev/null 2>&1 || true
    rm -rf "$WORK"
}
trap cleanup EXIT

step() { printf '\n==> %s\n' "$*"; }

step "build the $RUNG migrator"
git -C "$ROOT" worktree add --detach "$WORK/rung" "$RUNG" >/dev/null
(
    cd "$WORK/rung"
    python3 "$SCRIPTS/ci-strip-cargo-config.py"
    rustup show >/dev/null
    SQLX_OFFLINE=true cargo build --manifest-path crates/tests/Cargo.toml \
        -p systemprompt-test-migrate --target-dir "$WORK/target"
)
RELEASED="$WORK/target/debug/systemprompt-test-migrate"

for db in "$UP" "$FRESH"; do
    psql "$SERVER/postgres" -XAtq -v ON_ERROR_STOP=1 \
        -c "DROP DATABASE IF EXISTS \"$db\" WITH (FORCE)" -c "CREATE DATABASE \"$db\""
done

step "install $RUNG into $UP — a database left by that release"
DATABASE_URL="$SERVER/$UP" "$RELEASED"

step "upgrade $UP with the current tree — the path every deployed instance takes"
DATABASE_URL="$SERVER/$UP" "$CURRENT"
step "boot the current tree against $UP again — a restart must be a no-op"
DATABASE_URL="$SERVER/$UP" "$CURRENT"

step "fresh install of the current tree into $FRESH"
DATABASE_URL="$SERVER/$FRESH" "$CURRENT"

snapshot() { psql "$SERVER/$1" -XAtq -v ON_ERROR_STOP=1 -f "$CATALOG" | LC_ALL=C sort -u; }
snapshot "$UP" > "$WORK/upgraded.txt"
snapshot "$FRESH" > "$WORK/fresh.txt"

step "compare the upgraded schema with a fresh install"
if ! diff -u --label "fresh install" --label "upgraded from $RUNG" \
        "$WORK/fresh.txt" "$WORK/upgraded.txt" > "$WORK/diff.txt"; then
    grep -E '^[-+][^-+]' "$WORK/diff.txt" || true
    echo
    echo "::error::A database left by $RUNG does not upgrade to the schema a fresh install has."
    echo "'-' lines exist only on a fresh install, '+' lines only after the upgrade."
    echo "Fix the migration (for established databases) or the declarative schema"
    echo "(for fresh installs) — never both by hand. A migration already on next is"
    echo "immutable: correct it with a new one."
    exit 1
fi
echo "    $(wc -l < "$WORK/fresh.txt") schema objects identical"

for db in "$UP" "$FRESH"; do
    psql "$SERVER/postgres" -XAtq -c "DROP DATABASE IF EXISTS \"$db\" WITH (FORCE)" || true
done
