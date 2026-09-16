#!/usr/bin/env bash
# Async traits are native `async fn`; `#[async_trait]` exists only for
# `dyn`-compatibility and the trait must say so (CLAUDE.md § Rust Standards).
#
# Two rules over the production crates, the facade and the bridge:
#
#   dyn-unused        `#[async_trait] trait T` with no `dyn T` anywhere in the
#                     repository (tests included: a test may be the only dyn
#                     user, which still justifies the attribute) and no
#                     supertrait role in a trait that is dyn-used. Convert it
#                     to native `async fn`.
#   dyn-undocumented  the contiguous `///` block directly above the attribute
#                     (or, for a trait with no `///`, the file's `//!` head)
#                     does not contain the word `dyn`. Say why the trait is
#                     object-safe.
#
#   scripts/lint-async-trait.sh            # gate
#   scripts/lint-async-trait.sh --count    # hit count only
set -uo pipefail
cd "$(dirname "$0")/.."
command -v rg >/dev/null || { echo "lint-async-trait: ripgrep required" >&2; exit 2; }

COUNT=0
[ "${1:-}" = "--count" ] && COUNT=1

PROD_GLOBS=(-g '*.rs' -g '!crates/tests/**' -g '!**/target/**' -g '!**/build.rs')
PROD_ROOTS=(crates systemprompt/src bin/bridge/src)
ALL_ROOTS=(crates systemprompt/src bin/bridge/src)

declare -A trait_file trait_line dyn_used
order=()
while IFS=: read -r file line; do
    [ -n "$file" ] || continue
    name=$(awk -v start="$line" '
        NR > start && /^[[:space:]]*(pub(\([a-z]+\))?[[:space:]]+)?(unsafe[[:space:]]+)?trait[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
            match($0, /trait[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/)
            print substr($0, RSTART + 6, RLENGTH - 6); exit
        }
        NR > start + 4 { exit }
    ' "$file" | tr -d '[:space:]')
    [ -n "$name" ] || continue
    trait_file[$name]="$file"
    trait_line[$name]="$line"
    order+=("$name")
    uses=$(rg -c --no-messages -g '*.rs' -g '!**/target/**' \
        -e "dyn[[:space:]]+([A-Za-z0-9_]+::)*${name}\b" \
        "${ALL_ROOTS[@]}" | awk -F: '{ n += $NF } END { print n + 0 }')
    [ "$uses" -gt 0 ] && dyn_used[$name]=1
done < <(rg -n --no-heading --color=never "${PROD_GLOBS[@]}" \
    -e '^[[:space:]]*#\[(async_trait::)?async_trait(\([^)]*\))?\]' "${PROD_ROOTS[@]}" \
    | awk -F: '{ print $1 ":" $2 }' | sort -u)
scanned=${#order[@]}

# A supertrait of a dyn-used trait is dyn-used too (`dyn Sub` carries the
# supertrait's vtable), transitively; a supertrait of an unused trait is not.
changed=1
while [ "$changed" -eq 1 ]; do
    changed=0
    for name in "${order[@]}"; do
        [ -z "${dyn_used[$name]:-}" ] || continue
        for sub in "${!dyn_used[@]}"; do
            if rg -q --no-messages -g '*.rs' -g '!**/target/**' \
                -e "trait[[:space:]]+${sub}[[:space:]]*(<[^>]*>)?[[:space:]]*:[^{]*\b${name}\b" "${ALL_ROOTS[@]}"; then
                dyn_used[$name]=1; changed=1; break
            fi
        done
    done
done

hits=""
for name in "${order[@]}"; do
    file="${trait_file[$name]}"; line="${trait_line[$name]}"
    if [ -z "${dyn_used[$name]:-}" ]; then
        hits+="$file:$line: dyn-unused: #[async_trait] on $name but nothing takes dyn $name — use native async fn"$'\n'
        continue
    fi
    documented=$(awk -v start="$line" '
        NR < start && /^[[:space:]]*\/\/\// { if (!in_doc) doc = ""; in_doc = 1; doc = doc $0 "\n"; next }
        NR < start && /^[[:space:]]*\/\/!/ { head = head $0 "\n"; next }
        NR < start && /^[[:space:]]*#\[/ { next }
        NR < start && /^[[:space:]]*$/ { next }
        NR < start { in_doc = 0 }
        NR == start {
            block = in_doc ? doc : ""
            if (block ~ /\<dyn\>/ || (block == "" && head ~ /\<dyn\>/)) print "yes"; else print "no"
            exit
        }
    ' "$file")
    if [ "$documented" != "yes" ]; then
        hits+="$file:$line: dyn-undocumented: #[async_trait] on $name without a /// (or //! head) line naming the dyn requirement"$'\n'
    fi
done

[ "$scanned" -gt 0 ] || { echo "lint-async-trait: no #[async_trait] found — scope broken?" >&2; exit 1; }
if [ "$COUNT" -eq 1 ]; then printf '%s' "$hits" | grep -c .; exit 0; fi
if [ -n "$hits" ]; then
    echo "lint-async-trait: #[async_trait] is only for dyn-compatibility, and the trait must document it:" >&2
    printf '%s' "$hits" >&2
    exit 1
fi
echo "lint-async-trait: OK ($scanned async traits)"
