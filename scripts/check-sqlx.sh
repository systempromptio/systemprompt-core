#!/usr/bin/env bash
set -euo pipefail

# Match sqlx::query( and sqlx::query_{as,scalar,file,file_as,file_scalar,with,...}(
pattern='sqlx::query[a-z_]*(::<[^>]*>)?\('

# Allowlist entries live in scripts/sqlx-allowlist.txt (or the file named by
# `--allowlist`); each is a path: a directory (trailing `/`) exempts the tree
# under it, a file exempts that file. A stale carve-out is removed, not kept.
#
#   scripts/check-sqlx.sh                        # core roots + allowlist
#   scripts/check-sqlx.sh [--allowlist FILE] ROOT...
#
# Without roots the script runs from the repository root over `crates/`; with
# roots, paths (and allowlist entries) resolve against the caller's working
# directory.
ALLOWLIST=""
ROOTS=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --allowlist) ALLOWLIST="$2"; shift 2 ;;
        --allowlist=*) ALLOWLIST="${1#--allowlist=}"; shift ;;
        -*) echo "usage: $0 [--allowlist FILE] [ROOT...]" >&2; exit 2 ;;
        *) ROOTS+=("$1"); shift ;;
    esac
done
if [ "${#ROOTS[@]}" -eq 0 ]; then
    cd "$(git rev-parse --show-toplevel)"
    ROOTS=(crates/)
    ALLOWLIST="${ALLOWLIST:-scripts/sqlx-allowlist.txt}"
fi

allowlist=()
if [ -n "$ALLOWLIST" ]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="$(printf '%s' "$line" | sed 's/[[:space:]]*$//')"
        [ -n "$line" ] && allowlist+=("$line")
    done < "$ALLOWLIST"
fi

runtime_hits() {
    rg -n --glob '*.rs' -e "$pattern" -- "$@" 2>/dev/null \
        | grep -Ev 'sqlx::query[a-z_]*!' || true
}

stale=""
allowlist_re=""
for entry in "${allowlist[@]}"; do
    if [[ ! -e "${entry}" ]]; then
        stale+="  ${entry} (path does not exist)"$'\n'
    elif [[ -z "$(runtime_hits "${entry}")" ]]; then
        stale+="  ${entry} (no runtime sqlx::query call left)"$'\n'
    fi
    escaped=$(printf '%s' "${entry}" | sed 's/[.[\*^$()+?{|]/\\&/g')
    if [[ "${entry}" == */ ]]; then
        allowlist_re+="${allowlist_re:+|}^${escaped}"
    else
        allowlist_re+="${allowlist_re:+|}^${escaped}:"
    fi
done

if [[ -n "${stale}" ]]; then
    echo "❌ Stale scripts/check-sqlx.sh allowlist entries — remove them:" >&2
    printf '%s' "${stale}" >&2
    exit 1
fi

if [ "${#allowlist[@]}" -gt 0 ]; then
    hits=$(runtime_hits "${ROOTS[@]}" | grep -Ev "(${allowlist_re})" || true)
else
    hits=$(runtime_hits "${ROOTS[@]}")
fi

if [[ -n "${hits}" ]]; then
    echo "❌ Unverified sqlx::query calls found outside the allowlist:" >&2
    echo "${hits}" >&2
    echo "" >&2
    echo "Use sqlx::query!() / query_as!() / query_scalar!() (compile-time verified)." >&2
    echo "If the call must stay dynamic, add the path to scripts/check-sqlx.sh allowlist with justification." >&2
    exit 1
fi

echo "✅ No unverified sqlx::query calls outside the allowlist."
