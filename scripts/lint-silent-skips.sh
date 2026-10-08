#!/usr/bin/env bash
# Reject tests that return early on a missing prerequisite without saying so.
#
# A `return;` (or `return Ok(());`) inside a `#[test]` / `#[tokio::test]` body
# before the test's last statement is a self-skip: the test passes without
# exercising anything. Every such return must carry a `// skip-ok: <reason>`
# comment within the WINDOW lines above it, inside the same test body (the
# convention is the comment on the line before the `let ... else { return; }`
# that decides the skip). A marker outside a test body, or in another test,
# covers nothing. String and char literals and `//` comments are blanked
# before matching, so `"... { return; }"` inside an assertion is not a return.
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
        function hashes(n,    h) { h = ""; while (n-- > 0) h = h "#"; return h }
        # The line with string/char literal bodies and // comments blanked.
        # str (0 none, 1 plain, 2 raw with rawh hashes) carries across lines,
        # so a multi-line literal is blanked too.
        function code_only(s,    out, i, n, ch, pv, k, h) {
            out = ""; n = length(s); i = 1
            while (i <= n) {
                ch = substr(s, i, 1)
                if (str == 1) {
                    if (ch == "\\") { i += 2; continue }
                    if (ch == "\"") { str = 0; out = out ch }
                    i++; continue
                }
                if (str == 2) {
                    if (ch == "\"" && substr(s, i + 1, rawh) == hashes(rawh)) {
                        str = 0; out = out ch; i += 1 + rawh; continue
                    }
                    i++; continue
                }
                if (ch == "/" && substr(s, i + 1, 1) == "/") break
                if (ch == "\"") { str = 1; out = out ch; i++; continue }
                pv = (i > 1) ? substr(s, i - 1, 1) : ""
                if (ch == "r" && (pv == "b" || pv !~ /[A-Za-z0-9_]/)) {
                    k = i + 1; h = 0
                    while (substr(s, k, 1) == "#") { h++; k++ }
                    if (substr(s, k, 1) == "\"") { str = 2; rawh = h; out = out "\""; i = k + 1; continue }
                }
                if (ch == "\047") {
                    if (substr(s, i + 2, 1) == "\047") { out = out "\047\047"; i += 3; continue }
                    if (substr(s, i + 1, 1) == "\\") {
                        k = index(substr(s, i + 3), "\047")
                        if (k > 0) { out = out "\047\047"; i += 3 + k; continue }
                    }
                }
                out = out ch; i++
            }
            return out
        }
        FNR == 1 { str = 0; in_test = 0; pending = 0; last_ok = 0 }
        {
            line = code_only($0)
            if (line ~ /^[[:space:]]*#\[(tokio::|sqlx::|rstest|test)/) pending = 1
        }
        pending && line ~ /^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]/ { in_test = 1; pending = 0; depth = 0; last_ok = 0 }
        {
            if (!in_test) next
            if ($0 ~ /skip-ok:/) last_ok = FNR
            o = gsub(/\{/, "{", line); c = gsub(/\}/, "}", line)
            depth += o - c
            if (line ~ /(^|[^A-Za-z0-9_])return[[:space:]]*(Ok\(\(\)\))?[[:space:]]*;/ && depth >= 1) {
                if (!(last_ok && FNR - last_ok <= W))
                    print FILENAME ":" FNR ": early return in a test without a `// skip-ok: <reason>` within " W " lines"
            }
            if (depth <= 0 && o + c > 0) { in_test = 0; last_ok = 0 }
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
