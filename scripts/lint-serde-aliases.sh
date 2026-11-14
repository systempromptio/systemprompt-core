#!/usr/bin/env bash
# One name per field: a `#[serde(alias = "…")]` keeps a retired key alive as a
# second spelling and hides the rename from every consumer. A field has one
# wire name (`rename`); an alias is accepted only for a vendor format we do
# not own, listed in its package's `gates/lint-serde-aliases.allow` as
# `path:field — reason` (path relative to the package).
#
# The same gate keeps the default ports in one place: a bare `8080` or `5432`
# in a production source outside crates/shared/models/src/defaults.rs fails.
# Compose templates render `DEFAULT_POSTGRES_PORT`; clap defaults use
# `default_value_t = DEFAULT_*`; a comment or a test is not scanned.
#
#   scripts/lint-serde-aliases.sh                        # core roots + per-unit allowlists
#   scripts/lint-serde-aliases.sh [--allowlist FILE] ROOT...
#   scripts/lint-serde-aliases.sh --paths P...           # only files under P; stale
#                                                        # entries not judged
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"

root="$GATE_ROOT"
allowlist=""
roots=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --allowlist) allowlist="$2"; shift 2 ;;
        --allowlist=*) allowlist="${1#--allowlist=}"; shift ;;
        -h|--help) awk 'NR > 1 && !/^#/ { exit } NR > 1' "$0"; exit 0 ;;
        *) roots+=("$1"); shift ;;
    esac
done
gate_require_one_scope "${#roots[@]}"
if [ "${#roots[@]}" -eq 0 ]; then
    mapfile -t roots < <(gate_roots "$root/crates" "$root/systemprompt/src" "$root/bin/bridge/src")
    gate_pass_if_empty lint-serde-aliases "${#roots[@]}"
fi

declare -A allowed=()
gate_load_allowlist ALLOW_ENTRIES entries lint-serde-aliases "$allowlist"
label="$(gate_allowlist_label lint-serde-aliases "$allowlist")"
for line in "${ALLOW_ENTRIES[@]}"; do
    key="${line%% — *}"
    if [ "$key" = "$line" ] || [ -z "$(printf '%s' "${line#* — }" | tr -d '[:space:]')" ]; then
        echo "lint-serde-aliases: $label: '$line' has no reason (write: path:field — reason)" >&2
        exit 1
    fi
    allowed["$key"]=1
done

list_sources() {
    gate_tree_files "${roots[@]}" | grep -E '\.rs$' | grep -Ev '(^|/)(crates/tests|target|tests)/|/build\.rs$' | sort -u
}

rel_of() {
    case "$1" in "$root"/*) rel="${1#"$root"/}" ;; *) rel="$1" ;; esac
}

fail=0
declare -A seen
sources="$(list_sources)"

# Rule 1: serde aliases. A `#[serde(...)]` attribute may span lines; the
# field or variant it decorates is the first item line after it closes.
alias_hits() {
    tr '\n' '\0' | xargs -0 -r awk '
            function item(line) {
                sub(/^[[:space:]]*(pub(\([a-z]+\))?[[:space:]]+)?/, "", line)
                sub(/[[:space:]]*[:,({].*$/, "", line)
                return line
            }
            FNR == 1 { in_serde = 0; hit = 0; pending = 0 }
            /^[[:space:]]*\/\// { next }
            {
                if (!in_serde && $0 ~ /#\[serde\(/) { in_serde = 1; hit = 0 }
                if (in_serde && $0 ~ /(^|[^A-Za-z0-9_])alias[[:space:]]*=/) { hit = FNR }
                if (in_serde && $0 ~ /\)\]/) { in_serde = 0; if (hit) pending = hit; next }
                if (pending && !in_serde && $0 !~ /^[[:space:]]*#\[/ && $0 ~ /^[[:space:]]*(pub(\([a-z]+\))?[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*[[:space:]]*([:,({]|$)/) {
                    print FILENAME ":" pending ":" item($0); pending = 0
                }
            }
        ' 2>/dev/null || true
}

while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file="${hit%%:*}"
    rest="${hit#*:}"
    line="${rest%%:*}"
    field="${rest#*:}"
    rel_of "$file"
    key="$rel:$field"
    if [ -n "${allowed[$key]:-}" ]; then
        seen["$key"]=1
    else
        echo "lint-serde-aliases: $rel:$line -- serde alias on \`$field\`; a field has one wire name" >&2
        fail=1
    fi
done < <(printf '%s\n' "$sources" | alias_hits)

gate_full_mode && for key in "${!allowed[@]}"; do
    if [ -z "${seen[$key]:-}" ]; then
        echo "lint-serde-aliases: allowlist entry \`$key\` matches nothing -- remove it" >&2
        fail=1
    fi
done

# Rule 2: magic ports. A `8080`/`5432` token in code (not in a comment) outside
# the defaults module fails.
port_pattern='(^|[^0-9A-Za-z_.])(8080|5432)([^0-9A-Za-z_]|$)'
while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file="${hit%%:*}"
    rest="${hit#*:}"
    line="${rest%%:*}"
    text="${rest#*:}"
    rel_of "$file"
    case "$rel" in crates/shared/models/src/defaults.rs) continue ;; esac
    stripped="${text%%//*}"
    [[ "$stripped" =~ $port_pattern ]] || continue
    echo "lint-serde-aliases: $rel:$line -- port literal; use systemprompt_models::defaults::{DEFAULT_API_PORT, DEFAULT_API_URL, DEFAULT_POSTGRES_PORT}" >&2
    fail=1
done < <(printf '%s\n' "$sources" | xargs -r grep -nHE '(^|[^0-9A-Za-z_.])(8080|5432)([^0-9A-Za-z_]|$)' 2>/dev/null || true)

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "lint-serde-aliases: OK (no unlisted serde aliases or port literals under ${roots[*]}; ${#allowed[@]} allowlisted)"
