#!/usr/bin/env bash
# `serde_json::Value` in a struct field, fn parameter or return type of a
# production crate is a protocol boundary and must say so: the same line or
# the line above carries `// JSON: <reason>`. Everything else is typed.
#
# Carve-outs are the surfaces where the wire shape *is* the type and a per-line
# annotation would repeat the module head on every field:
#   **/models/a2a/protocol/**                 A2A JSON-RPC protocol objects
#   crates/infra/database/src/admin/**        the admin SQL console rows
#   crates/infra/database/src/services/postgres/**   runtime query results
#   field names input_schema | output_schema | structured_content (MCP schema)
# `crates/shared/models/src/wire/**` (provider wire shapes) is deliberately
# NOT carved out: each Value there names the upstream spec it mirrors.
#
# Only files that import `serde_json::Value` (or alias it as `JsonValue`) are
# considered, so a domain `Value` type is not confused with JSON. `let`, `use`,
# `const`, `static` and comment lines are skipped: the rule is about
# signatures, not locals.
#
#   scripts/lint-json-value.sh            # gate
#   scripts/lint-json-value.sh --count    # hit count only
set -uo pipefail
cd "$(dirname "$0")/.."

COUNT=0
[ "${1:-}" = "--count" ] && COUNT=1

hits=""
scanned=0
while IFS= read -r file; do
    case "$file" in
        crates/tests/*|*/build.rs) continue ;;
        */models/a2a/protocol/*) continue ;;
        crates/infra/database/src/admin/*) continue ;;
        crates/infra/database/src/services/postgres/*) continue ;;
    esac
    [ -e "$file" ] || continue
    grep -qE 'serde_json::(Value|Map)|use serde_json::\{[^}]*\bValue\b|JsonValue' "$file" || continue
    scanned=$((scanned + 1))
    found=$(awk '
        function is_sig(text) {
            return text ~ /(:[[:space:]]*&?(mut[[:space:]]+)?|->[[:space:]]*&?)((Option|Vec|Result|Box|Arc|HashMap|BTreeMap|Map|IndexMap|Cow)<[^>]*)?(serde_json::Value|JsonValue|Value)([^:A-Za-z0-9_]|$)/
        }
        {
            text = $0
            annotated = (text ~ /\/\/ JSON:/) || (prev ~ /\/\/ JSON:/)
            skip = (text ~ /^[[:space:]]*(\/\/|let |use |const |static )/) \
                || (text ~ /(input_schema|output_schema|structured_content)[[:space:]]*:/)
            if (!skip && !annotated && is_sig(text)) print FILENAME ":" FNR ":" text
            prev = text
        }
    ' "$file")
    [ -n "$found" ] && hits+="$found"$'\n'
done < <(git ls-files -co --exclude-standard 'crates/*.rs' 'crates/**/*.rs' 'systemprompt/src/*.rs' 'systemprompt/src/**/*.rs' 'bin/bridge/src/*.rs' 'bin/bridge/src/**/*.rs' | sort -u)

[ "$scanned" -gt 0 ] || { echo "lint-json-value: no serde_json users scanned — scope broken?" >&2; exit 1; }
if [ "$COUNT" -eq 1 ]; then printf '%s' "$hits" | grep -c .; exit 0; fi
if [ -n "$hits" ]; then
    echo "lint-json-value: serde_json::Value in a signature needs '// JSON: <reason>' on the line above, or a typed struct:" >&2
    printf '%s' "$hits" >&2
    exit 1
fi
echo "lint-json-value: OK ($scanned files use serde_json::Value)"
