#!/usr/bin/env bash
# One name per field: a `#[serde(alias = "…")]` keeps a retired key alive as a
# second spelling and hides the rename from every consumer. A field has one
# wire name (`rename`); an alias is accepted only for a vendor format we do
# not own, listed in the allowlist as `path:field — reason`.
#
# The same gate keeps the default ports in one place: a bare `8080` or `5432`
# in a production source outside crates/shared/models/src/defaults.rs fails.
# Compose templates render `DEFAULT_POSTGRES_PORT`; clap defaults use
# `default_value_t = DEFAULT_*`; a comment or a test is not scanned.
#
#   scripts/lint-serde-aliases.sh                        # core roots + allowlist
#   scripts/lint-serde-aliases.sh [--allowlist FILE] ROOT...
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
allowlist="$root/scripts/serde-aliases-allowlist.txt"
roots=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --allowlist) allowlist="$2"; shift 2 ;;
        --allowlist=*) allowlist="${1#--allowlist=}"; shift ;;
        -h|--help) sed -n '2,13p' "$0"; exit 0 ;;
        *) roots+=("$1"); shift ;;
    esac
done
if [ "${#roots[@]}" -eq 0 ]; then
    roots=("$root/crates" "$root/systemprompt/src" "$root/bin/bridge/src")
fi

declare -A allowed
while IFS= read -r line; do
    line="${line%%#*}"
    line="$(printf '%s' "$line" | sed 's/[[:space:]]*$//')"
    [ -n "$line" ] || continue
    key="${line%% — *}"
    if [ "$key" = "$line" ] || [ -z "$(printf '%s' "${line#* — }" | tr -d '[:space:]')" ]; then
        echo "lint-serde-aliases: $allowlist: '$line' has no reason (write: path:field — reason)" >&2
        exit 1
    fi
    allowed["$key"]=1
done < "$allowlist"

list_sources() {
    local r
    for r in "${roots[@]}"; do
        if git -C "$r" rev-parse --show-toplevel >/dev/null 2>&1; then
            git -C "$r" ls-files -co --exclude-standard -- '*.rs' | sed "s|^|$r/|"
        else
            find "$r" -name '*.rs' -type f
        fi
    done | grep -Ev '(^|/)(crates/tests|target|tests)/|/build\.rs$' | sort -u \
        | while IFS= read -r f; do [ -f "$f" ] && printf '%s\n' "$f"; done
}

rel_of() {
    local file="$1"
    case "$file" in "$root"/*) printf '%s' "${file#"$root"/}" ;; *) printf '%s' "$file" ;; esac
}

fail=0
declare -A seen
sources="$(list_sources)"

# Rule 1: serde aliases. A `#[serde(...)]` attribute may span lines; the
# field or variant it decorates is the first item line after it closes.
alias_hits() {
    local file
    while IFS= read -r file; do
        awk -v f="$file" '
            function item(line) {
                sub(/^[[:space:]]*(pub(\([a-z]+\))?[[:space:]]+)?/, "", line)
                sub(/[[:space:]]*[:,({].*$/, "", line)
                return line
            }
            /^[[:space:]]*\/\// { next }
            {
                if (!in_serde && $0 ~ /#\[serde\(/) { in_serde = 1; hit = 0 }
                if (in_serde && $0 ~ /(^|[^A-Za-z0-9_])alias[[:space:]]*=/) { hit = NR }
                if (in_serde && $0 ~ /\)\]/) { in_serde = 0; if (hit) pending = hit; next }
                if (pending && !in_serde && $0 !~ /^[[:space:]]*#\[/ && $0 ~ /^[[:space:]]*(pub(\([a-z]+\))?[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*[[:space:]]*([:,({]|$)/) {
                    print f ":" pending ":" item($0); pending = 0
                }
            }
        ' "$file" 2>/dev/null || true
    done
}

while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file="${hit%%:*}"
    rest="${hit#*:}"
    line="${rest%%:*}"
    field="${rest#*:}"
    rel="$(rel_of "$file")"
    key="$rel:$field"
    if [ -n "${allowed[$key]:-}" ]; then
        seen["$key"]=1
    else
        echo "lint-serde-aliases: $rel:$line -- serde alias on \`$field\`; a field has one wire name" >&2
        fail=1
    fi
done < <(printf '%s\n' "$sources" | alias_hits)

for key in "${!allowed[@]}"; do
    if [ -z "${seen[$key]:-}" ]; then
        echo "lint-serde-aliases: allowlist entry \`$key\` matches nothing -- remove it" >&2
        fail=1
    fi
done

# Rule 2: magic ports. A `8080`/`5432` token in code (not in a comment) outside
# the defaults module fails.
while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file="${hit%%:*}"
    rest="${hit#*:}"
    line="${rest%%:*}"
    text="${rest#*:}"
    rel="$(rel_of "$file")"
    case "$rel" in crates/shared/models/src/defaults.rs) continue ;; esac
    stripped="$(printf '%s' "$text" | sed -E 's://.*$::')"
    printf '%s' "$stripped" | grep -qE '(^|[^0-9A-Za-z_.])(8080|5432)([^0-9A-Za-z_]|$)' || continue
    echo "lint-serde-aliases: $rel:$line -- port literal; use systemprompt_models::defaults::{DEFAULT_API_PORT, DEFAULT_API_URL, DEFAULT_POSTGRES_PORT}" >&2
    fail=1
done < <(printf '%s\n' "$sources" | xargs -r grep -nHE '(^|[^0-9A-Za-z_.])(8080|5432)([^0-9A-Za-z_]|$)' 2>/dev/null || true)

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "lint-serde-aliases: OK (no unlisted serde aliases or port literals under ${roots[*]}; ${#allowed[@]} allowlisted)"
