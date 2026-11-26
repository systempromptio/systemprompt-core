#!/usr/bin/env bash
# The composition proof: every line of scripts/build-compositions.txt builds;
# each line flagged `boot` links the `systemprompt` binary, runs
# `systemprompt --help` (which declares every member and builds the command
# tree) and declares exactly its scripts/composition-members.txt row (the
# facade's `members` example). Then the closure of every line is measured with
# `scripts/build-report.sh --closure-only` (workspace packages, total
# packages, capability-unit packages) and the line flagged `floor` must link
# no capability-unit package of scripts/units.txt.
#
# --schema also installs the schema of every booted line on a fresh database
# (crates/tests/integration/database, composition_schemas); it needs
# DATABASE_URL (compile-time sqlx validation of the test workspace) and
# TEST_DATABASE_URL or PGPASSWORD/~/.pgpass for the disposable databases.
# CI runs the script without it: the test shards run that test.
#
# Usage: scripts/compositions.sh [--only NAME] [--offline] [--schema] [REPORT.md]
#   REPORT.md defaults to target/build-report/compositions.md (JSON beside it).
set -euo pipefail
cd "$(dirname "$0")/.."

only=""
offline=()
schema=false
out="target/build-report/compositions.md"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --only) only="${2:?--only needs a composition name}"; shift ;;
        --offline) offline=(--offline) ;;
        --schema) schema=true ;;
        -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
        -*) echo "unknown flag: $1" >&2; exit 2 ;;
        *) out="$1" ;;
    esac
    shift
done
command -v jq >/dev/null || { echo 'jq is not installed' >&2; exit 2; }
export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"

table=scripts/build-compositions.txt
members=scripts/composition-members.txt
mapfile -t rows < <(grep -vE '^\s*(#|$)' "$table")
[ "${#rows[@]}" -gt 0 ] || { echo "$table lists no composition" >&2; exit 1; }
[ "$(printf '%s\n' "${rows[@]}" | awk '$3 ~ /(^|,)floor(,|$)/' | wc -l)" -eq 1 ] \
    || { echo "$table must mark exactly one composition floor" >&2; exit 1; }

failed=()
for row in "${rows[@]}"; do
    read -r name features flags <<<"$row"
    [ -n "$only" ] && [ "$name" != "$only" ] && continue
    echo "== $name ($features)" >&2
    facade=(-p systemprompt --no-default-features --features "$features" "${offline[@]}" --locked)
    case ",${flags:-}," in
        *,boot,*)
            cargo build "${facade[@]}" --bin systemprompt
            cargo run -q "${facade[@]}" --bin systemprompt -- --help >/dev/null
            expected=$(awk -v name="$name" '$1 == name { print $2 }' "$members")
            if [ -z "$expected" ]; then
                echo "$members lists no members for $name" >&2
                failed+=("$name: no member row")
                continue
            fi
            actual=$(cargo run -q "${facade[@]}" --example members | paste -sd, -)
            if [ "$actual" != "$expected" ]; then
                echo "$name declares members other than $members lists" >&2
                diff <(tr , '\n' <<<"$expected") <(tr , '\n' <<<"$actual") >&2 || true
                failed+=("$name: member set")
            fi
            ;;
        *)
            cargo build "${facade[@]}" --lib
            ;;
    esac
done

report=(--closure-only "${offline[@]}")
[ -n "$only" ] && report+=(--only "$only")
bash scripts/build-report.sh "${report[@]}" "$out" >/dev/null
json="${out%.md}.json"
while read -r name linked; do
    echo "$name is the floor composition and links capability packages: $linked" >&2
    failed+=("$name: floor")
done < <(jq -r '.compositions[] | select(.flags | index("floor")) | select((.capability_packages | length) > 0)
    | "\(.name) \(.capability_packages | join(","))"' "$json")

if $schema; then
    [ -n "${DATABASE_URL:-}" ] || { echo '--schema needs DATABASE_URL' >&2; exit 2; }
    (cd crates/tests && SQLX_OFFLINE=false cargo test "${offline[@]}" --locked \
        -p systemprompt-database-integration-tests --lib composition_schemas::) || failed+=("schema install")
fi

cat "$out"
if [ "${#failed[@]}" -gt 0 ]; then
    printf 'compositions: FAIL — %s\n' "${failed[@]}" >&2
    exit 1
fi
echo "compositions: OK — every line builds, booted lines declare their members, the floor line links no capability" >&2
