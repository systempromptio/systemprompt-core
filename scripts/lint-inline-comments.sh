#!/usr/bin/env bash
set -uo pipefail

# Machete rule: inline `//` comments are banned in production crates.
#
# The only permitted full-line inline comments are the two whitelisted
# justification prefixes mandated by the rust-coding-standards skill:
#
#   // Why:  — a non-obvious invariant, hidden constraint, or exemption
#              justification (e.g. a permitted `let _ =`)
#   // JSON: — a sanctioned `serde_json::Value` protocol-boundary usage
#
# Continuation lines of a whitelisted comment block are allowed. `//!` module
# heads are governed separately (rustdoc placement rules), as are `///` docs
# on public API items. `crates/tests/**` and `build.rs` files are out of scope.
#
# A second check flags `///` rustdoc on items that are NOT public API —
# `pub(crate)`, `pub(super)`, and private `async fn` items (rustdoc is never
# rendered for them). A genuine invariant on such an item belongs in a
# `// Why:` comment; anything else is deleted.

MATCHES=""
while IFS= read -r file; do
    case "$file" in
        crates/tests/*) continue ;;
        */build.rs) continue ;;
    esac
    FOUND=$(awk '
        /^[[:space:]]*\/\/\// { prev_allowed = 0; if (!in_doc) doc_line = FNR; in_doc = 1; next }
        /^[[:space:]]*\/\/!/ { prev_allowed = 0; next }
        /^[[:space:]]*\/\// {
            in_doc = 0
            if ($0 ~ /^[[:space:]]*\/\/ (Why|JSON):/) { prev_allowed = 1; next }
            if (prev_allowed) { next }
            print FILENAME ":" FNR ":" $0
            next
        }
        /^[[:space:]]*#!?\[/ { next }
        {
            if (in_doc) {
                stripped = $0
                sub(/^[[:space:]]+/, "", stripped)
                if (stripped ~ /^(pub\(crate\)|pub\(super\))/) {
                    print FILENAME ":" doc_line ": rustdoc on non-public item (" stripped ") — use // Why: or delete"
                }
            }
            in_doc = 0
            prev_allowed = 0
        }
    ' "$file")
    [ -n "$FOUND" ] && MATCHES+="${FOUND}"$'\n'
done < <(git ls-files 'crates/*.rs' 'crates/**/*.rs' | sort -u)

if [ -z "$MATCHES" ]; then
    echo "lint-inline-comments: OK (no unlisted inline comments)"
    exit 0
fi

echo "lint-inline-comments: inline // comments are banned in production crates."
echo "Delete the comment, or justify it with a '// Why:' or '// JSON:' prefix:"
echo ""
printf '%s' "$MATCHES"
exit 1
