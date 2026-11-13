#!/usr/bin/env bash
# Reject unlisted process-global service state in the production crates.
#
# `AppContext` (and the server, gateway and bridge roots) own injected
# services; a `static` holding an `OnceLock`, `LazyLock`, `RwLock`, `Mutex`,
# `ArcSwap`, `Once`, `OnceCell` or an atomic is service state unless it is
# listed in the allowlist as `path:NAME — reason`. A reason is required; an
# entry whose static no longer exists fails so the list cannot go stale.
#
#   scripts/lint-core-globals.sh                       # core roots + allowlist
#   scripts/lint-core-globals.sh [--allowlist FILE] ROOT...
#   scripts/lint-core-globals.sh --paths P...          # only files under P; stale
#                                                      # entries not judged
#
# Roots are directories of Rust sources (tracked or not); `crates/tests/**`,
# `target/` and `build.rs` are never scanned.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"

root="$GATE_ROOT"
allowlist="$root/scripts/core-globals-allowlist.txt"
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
    mapfile -t roots < <(gate_roots "$root/crates" "$root/systemprompt/src")
    gate_pass_if_empty lint-core-globals "${#roots[@]}"
fi

pattern='^\s*(pub(\([a-z]+\))? )?static [A-Z_][A-Z0-9_]*: *(std::sync::|std::sync::atomic::|tokio::sync::)?(OnceLock|LazyLock|RwLock|Mutex|ArcSwap|Atomic[A-Za-z0-9]+|Once|OnceCell)\b'

declare -A allowed reason_of
while IFS= read -r line; do
    line="${line%%#*}"
    line="$(printf '%s' "$line" | sed 's/[[:space:]]*$//')"
    [ -n "$line" ] || continue
    key="${line%% — *}"
    if [ "$key" = "$line" ] || [ -z "$(printf '%s' "${line#* — }" | tr -d '[:space:]')" ]; then
        echo "lint-core-globals: $allowlist: '$line' has no reason (write: path:NAME — reason)" >&2
        exit 1
    fi
    allowed["$key"]=1
    reason_of["$key"]="${line#* — }"
done < "$allowlist"

list_sources() {
    gate_tree_files "${roots[@]}" | grep -E '\.rs$' | grep -Ev '(^|/)(crates/tests|target)/|/build\.rs$' | sort -u
}

fail=0
declare -A seen
while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file="${hit%%:*}"
    rest="${hit#*:}"
    line="${rest%%:*}"
    name="$(sed -E 's/^.*static ([A-Z_][A-Z0-9_]*):.*$/\1/' <<< "${rest#*:}")"
    rel="$file"
    case "$file" in "$root"/*) rel="${file#"$root"/}" ;; esac
    key="$rel:$name"
    if [ -n "${allowed[$key]:-}" ]; then
        seen["$key"]=1
    else
        echo "lint-core-globals: $rel:$line -- static \`$name\` holds service state; build it at the composition root and inject it" >&2
        fail=1
    fi
done < <(list_sources | xargs -r grep -nHE "$pattern" 2>/dev/null || true)

gate_full_mode && for key in "${!allowed[@]}"; do
    file="${key%:*}"
    name="${key##*:}"
    path="$file"
    case "$file" in /*) ;; *) path="$root/$file" ;; esac
    if [ -z "${seen[$key]:-}" ] && ! grep -qE "static $name:" "$path" 2>/dev/null; then
        echo "lint-core-globals: allowlist entry \`$key\` matches nothing -- remove it" >&2
        fail=1
    fi
done

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "lint-core-globals: OK (no unlisted process statics under ${roots[*]}; ${#allowed[@]} allowlisted)"
