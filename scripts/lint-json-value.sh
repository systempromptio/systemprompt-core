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
# A hit is a `Value` in type position: after a field/param `name:`, after `->`,
# or inside a tuple variant. Turbofish arguments (`from_str::<Value>`,
# `collect::<Map<_, Value>>`) and other crates' `Value` (`serde_yaml::Value`,
# `AnyValue`) are expressions or foreign types, not signatures.
#
# "The line above" looks through `#[...]` attributes (multi-line too) and doc
# lines, and a `// JSON:` note may continue over further `//` lines. A note
# directly above a multi-line `fn` covers its params and return type.
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
        function type_position(text, s,    i, c, depth) {
            depth = 0
            for (i = s - 1; i >= 1; i--) {
                c = substr(text, i, 1)
                if (c == ")" || c == "]") depth++
                else if (c == ">") {
                    if (i > 1 && substr(text, i - 1, 1) == "-") {
                        if (depth == 0) return 1
                        i--
                    } else depth++
                } else if (c == "(" || c == "[" || c == "<") {
                    if (depth > 0) { depth--; continue }
                    if (c == "<" && i > 2 && substr(text, i - 2, 2) == "::") return 0
                    if (c == "(" && substr(text, 1, i - 1) ~ /(^|[^A-Za-z0-9_])[A-Z][A-Za-z0-9_]*[[:space:]]*$/) return 1
                } else if (depth == 0 && c == ":") {
                    if (i > 1 && substr(text, i - 1, 1) == ":") { i--; continue }
                    if (substr(text, i + 1, 1) == ":") continue
                    return 1
                } else if (depth == 0 && (c == "=" || c == "{" || c == ";")) return 0
            }
            return 0
        }
        function is_sig(text,    rest, off, s, e) {
            rest = text; off = 0
            while (match(rest, /(serde_json::)?(Json)?Value/)) {
                s = off + RSTART; e = s + RLENGTH
                if ((s == 1 || substr(text, s - 1, 1) !~ /[A-Za-z0-9_:]/) \
                    && substr(text, e, 1) !~ /[A-Za-z0-9_:]/ \
                    && type_position(text, s)) return 1
                off = e - 1; rest = substr(text, e)
            }
            return 0
        }
        function brackets(text,    t, opened, closed) {
            t = text; opened = gsub(/\[/, "", t)
            t = text; closed = gsub(/\]/, "", t)
            return opened - closed
        }
        {
            text = $0
            marked = (text ~ /\/\/ JSON:/)
            if (attr_depth > 0) { attr_depth += brackets(text); next }
            if (text ~ /^[[:space:]]*#!?\[/) {
                attr_depth = brackets(text)
                if (attr_depth < 0) attr_depth = 0
                next
            }
            if (text ~ /^[[:space:]]*\/\/[\/!]/) next
            if (text ~ /^[[:space:]]*\/\//) { if (marked) pending = 1; next }
            if (text ~ /^[[:space:]]*$/) { pending = 0; in_sig = 0; next }
            annotated = marked || pending || in_sig
            skip = (text ~ /^[[:space:]]*(let |use |const |static )/) \
                || (text ~ /(input_schema|output_schema|structured_content)[[:space:]]*:/)
            if (!skip && !annotated && is_sig(text)) print FILENAME ":" FNR ":" text
            ends_sig = (text ~ /[{;][[:space:]]*(\/\/.*)?$/)
            if (in_sig && ends_sig) in_sig = 0
            else if ((pending || marked) && text ~ /(^|[^A-Za-z0-9_])fn[[:space:]]/ && !ends_sig) in_sig = 1
            pending = marked
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
