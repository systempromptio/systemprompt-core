#!/usr/bin/env bash
# A name from scripts/foreign-names.txt appears nowhere in the lifted device
# crates or their tests.
#
# The engine, device, harness and Drive crates were written from scratch. The
# projects read for behaviour were never copied and never named, so a hit is a
# copied identifier, a borrowed string or a note that names its source. The
# patterns live in scripts/foreign-names.txt, written so that file does not
# match itself; this script names none of them.
#
# Scope: every tracked file and every new, unignored one under the four lifted
# crates and their test crates, contents and paths, matched case-insensitively.
# No allowlist. The host integrations name the products they govern and are
# not in scope.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2

list=scripts/foreign-names.txt
scope=(
    crates/domain/engine
    crates/infra/device
    crates/app/harness
    crates/domain/drive
    crates/tests/unit/domain/engine
    crates/tests/unit/infra/device
    crates/tests/unit/app/harness
    crates/tests/unit/domain/drive
    crates/tests/unit/shared/models-device
)
[ -f "$list" ] || { echo "lint-foreign-names: $list is missing" >&2; exit 2; }

patterns=$(grep -vE '^[[:space:]]*(#|$)' "$list")
if [ -z "$patterns" ]; then
    echo "lint-foreign-names: $list lists no patterns; refusing to pass vacuously" >&2
    exit 2
fi

for dir in "${scope[@]}"; do
    [ -d "$dir" ] || { echo "lint-foreign-names: $dir is missing; the scope is out of step" >&2; exit 2; }
done

files=()
while IFS= read -r -d '' f; do
    [ -f "$f" ] && files+=("$f")
done < <(git ls-files -co --exclude-standard -z -- "${scope[@]}")
[ "${#files[@]}" -gt 0 ] || { echo "lint-foreign-names: no files listed; scope broken?" >&2; exit 2; }

paths=$(printf '%s\n' "${files[@]}" | grep -iE -f <(printf '%s\n' "$patterns") || true)
contents=$(printf '%s\0' "${files[@]}" | xargs -0 grep -InHiE -f <(printf '%s\n' "$patterns") 2>/dev/null || true)

if [ -n "$paths" ] || [ -n "$contents" ]; then
    echo "lint-foreign-names: a name listed in $list appears in the device crates:" >&2
    if [ -n "$paths" ]; then
        echo "  in a path:" >&2
        printf '%s\n' "$paths" | sed 's/^/    /' >&2
    fi
    if [ -n "$contents" ]; then
        echo "  in a file:" >&2
        printf '%s\n' "$contents" | sed 's/^/    /' >&2
    fi
    echo "Rename it or rewrite it in our own words; nothing here names a reference project." >&2
    exit 1
fi
echo "lint-foreign-names: OK (${#files[@]} files, $(printf '%s\n' "$patterns" | wc -l | tr -d ' ') pattern(s))"
