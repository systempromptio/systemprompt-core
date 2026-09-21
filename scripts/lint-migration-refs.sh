#!/usr/bin/env bash
# Pre-merge gate: a migration may not lean on an object only the declarative
# schema creates.
#
# The installer runs every extension's migrations BEFORE the dependent phase
# that applies declarative functions, views and triggers. A migration that
# calls `EXECUTE FUNCTION f()` or `ALTER TABLE … DISABLE TRIGGER t` where `f`
# or `t` lives only in schema/*.sql works on every database that has already
# booted on a core shipping that object, and fails the first upgrade from one
# that has not — the author never sees it because every workstation has. Two
# migrations shipped that way (mcp 010, marketplace 012) and each broke a
# self-host upgrade at "does not exist".
#
# For every function, view and trigger defined in a declarative schema file
# and in no migration at all, the gate rejects a migration that names it in a
# non-comment line, unless the same migration guards the reference by testing
# the catalog for it (a `'name'` literal alongside a `pg_proc` / `pg_trigger` /
# `pg_views` / `information_schema` lookup) or defines the object itself.
# Names referenced only in `-- comments` do not count.
#
# A migration that already shipped cannot be edited (its checksum is what
# every applied database holds), so a reference proven safe by history is
# listed in scripts/lint-migration-refs-allow.txt as `<path>:<name>` with the
# reason beside it. The list is for spent migrations only; a new one fixes
# the reference instead.

set -euo pipefail

ROOT="${1:-crates}"
ALLOW="$(dirname "$0")/lint-migration-refs-allow.txt"

# shellcheck disable=SC2207
schema_files=($(find "$ROOT" -type f -name '*.sql' \
    -path '*/schema/*' \
    -not -path '*/schema/migrations/*' \
    -not -path '*/target/*' \
    | sort))
# shellcheck disable=SC2207
migration_files=($(find "$ROOT" -type f -name '*.sql' \
    -path '*/schema/migrations/*' \
    -not -path '*/target/*' \
    | sort))

if [ ${#schema_files[@]} -eq 0 ] || [ ${#migration_files[@]} -eq 0 ]; then
    echo "lint-migration-refs: nothing to check under $ROOT"
    exit 0
fi

definition='CREATE[[:space:]]+(OR[[:space:]]+REPLACE[[:space:]]+)?(FUNCTION|VIEW|MATERIALIZED[[:space:]]+VIEW|TRIGGER)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)'

strip_comments() {
    sed -E 's/--.*$//' "$@"
}

defined_names() {
    strip_comments "$@" | grep -oEi "$definition" | awk '{print tolower($NF)}' | sort -u
}

declared=$(defined_names "${schema_files[@]}")
migrated=$(defined_names "${migration_files[@]}")
declarative_only=$(comm -23 <(echo "$declared") <(echo "$migrated"))

if [ -z "$declarative_only" ]; then
    echo "lint-migration-refs: no declarative-only objects"
    exit 0
fi

allowed() {
    [ -f "$ALLOW" ] && grep -qE "^$1:$2([[:space:]]|$)" "$ALLOW"
}

violations=0
for f in "${migration_files[@]}"; do
    body=$(strip_comments "$f")
    for name in $declarative_only; do
        if ! echo "$body" | grep -qiw -- "$name"; then
            continue
        fi
        if echo "$body" | grep -qiE "(pg_proc|pg_trigger|pg_views|pg_class|information_schema)" \
            && echo "$body" | grep -qi -- "'$name'"; then
            continue
        fi
        if allowed "$f" "$name"; then
            continue
        fi
        line=$(echo "$body" | grep -niw -m1 -- "$name" | cut -d: -f1)
        echo "$f:$line: references '$name', which only a declarative schema file creates — the dependent phase runs after migrations, so a database that has not booted on that schema fails here. Create it in a migration, guard the reference on the catalog, or leave it to the declarative schema."
        violations=$((violations + 1))
    done
done

if [ $violations -gt 0 ]; then
    echo "lint-migration-refs: $violations violation(s)"
    exit 1
fi
echo "lint-migration-refs: ok ($(echo "$declarative_only" | wc -l | tr -d ' ') declarative-only objects, ${#migration_files[@]} migrations)"
