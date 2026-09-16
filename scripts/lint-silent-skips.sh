#!/usr/bin/env bash
# Reject tests that return early on a missing prerequisite without saying so.
#
# A `return;` (or `return Ok(());`) inside a `#[test]` / `#[tokio::test]` body
# before the test's last statement is a self-skip: the test passes without
# exercising anything. Every such return must carry a `// skip-ok: <reason>`
# comment within the WINDOW lines above it (the convention is the comment on
# the line before the `let ... else { return; }` that decides the skip).
#
# An `eprintln!("Skipping …")` is not a marker: it is invisible to grep and to
# CI, and it is the form the marker replaces. Only `// skip-ok:` counts.
#
#   scripts/lint-silent-skips.sh [dir...]     # default: crates/tests
#   scripts/lint-silent-skips.sh --count      # hit count only
set -uo pipefail
cd "$(dirname "$0")/.."
WINDOW="${WINDOW:-8}"
COUNT=0
if [ "${1:-}" = "--count" ]; then COUNT=1; shift; fi
[ "$#" -gt 0 ] || set -- crates/tests
hits=""
scanned=0
while IFS= read -r file; do
    scanned=$((scanned + 1))
    found=$(awk -v W="$WINDOW" '
        /^[[:space:]]*#\[(tokio::|sqlx::|rstest|test)/ { pending = 1 }
        pending && /^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]/ { in_test = 1; pending = 0; depth = 0; last_ok = 0 }
        {
            if ($0 ~ /skip-ok:/) last_ok = FNR
            if (!in_test) next
            o = gsub(/\{/, "{"); c = gsub(/\}/, "}")
            depth += o - c
            if ($0 ~ /(^|[^A-Za-z0-9_])return[[:space:]]*(Ok\(\(\)\))?[[:space:]]*;/ && depth >= 1) {
                if (!(last_ok && FNR - last_ok <= W))
                    print FILENAME ":" FNR ": early return in a test without a `// skip-ok: <reason>` within " W " lines"
            }
            if (depth <= 0 && o + c > 0) { in_test = 0 }
        }
    ' "$file")
    [ -n "$found" ] && hits+="$found"$'\n'
done < <(git ls-files -co --exclude-standard "$@" | grep -E '\.rs$' | grep -vE '/(target)/')
[ "$scanned" -gt 0 ] || { echo "lint-silent-skips: no test sources scanned" >&2; exit 1; }
if [ "$COUNT" -eq 1 ]; then printf '%s' "$hits" | grep -c .; exit 0; fi
if [ -n "$hits" ]; then
    echo "lint-silent-skips: a test that returns early must say why (\`// skip-ok: <reason>\`):" >&2
    printf '%s' "$hits" >&2
    exit 1
fi
echo "lint-silent-skips: OK ($scanned files)"
