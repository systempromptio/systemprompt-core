#!/usr/bin/env bash
# Profiles are the source of truth; environment variables are a scoped escape
# hatch (CLAUDE.md § Configuration). A `std::env::var` reader anywhere else is
# an undocumented kill switch or fallback — 0.51.0 found one
# (`SYSTEMPROMPT_VERTEX_DISCOVERY`) only because a release went wrong.
#
# Every production file that reads the process environment must be listed in
# scripts/env-var-allowlist.txt with a reason naming the sanctioned variable
# or platform constraint. Tests (crates/tests/**) are not scanned.
#
# A reader is any of: `std::env::var*`, tracing's `EnvFilter::from_env` /
# `try_from_default_env` (RUST_LOG), and the bridge's `basedirs::env_dir(…)`
# wrapper — a wrapper is a reader at every call site, not only where it is
# defined, or the wrapper becomes the unlisted kill switch.
#
#   scripts/lint-env-vars.sh                          # core roots + allowlist
#   scripts/lint-env-vars.sh [--allowlist FILE] ROOT...
#
# Without roots the script runs from the repository root over the core source
# trees; with roots, paths (and allowlist entries) resolve against the caller's
# working directory. A directory literally named `tests` is never scanned.
set -euo pipefail

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
    cd "$(dirname "$0")/.."
    ALLOWLIST="${ALLOWLIST:-scripts/env-var-allowlist.txt}"
fi
if [ -z "$ALLOWLIST" ]; then
    echo "lint-env-vars: --allowlist FILE is required when roots are given" >&2
    exit 2
fi

list_sources() {
    if [ "${#ROOTS[@]}" -eq 0 ]; then
        git ls-files 'crates/*/*/src/**/*.rs' 'crates/*/*/src/*.rs' 'bin/bridge/src/**/*.rs' 'bin/bridge/src/*.rs'
    else
        local r
        for r in "${ROOTS[@]}"; do
            if git -C "$r" rev-parse --show-toplevel >/dev/null 2>&1; then
                git -C "$r" ls-files -co --exclude-standard -- '*.rs' | sed "s|^|${r%/}/|"
            else
                find "$r" -name '*.rs' -type f
            fi
        done
    fi | grep -Ev '(^|/)(tests|target)/|/build\.rs$' | while IFS= read -r f; do [ -f "$f" ] && printf '%s\n' "$f"; done
}

PATTERN='(std::)?env::(var|var_os|vars|vars_os)\(|EnvFilter::(try_from_default_env|from_env|from_default_env)\(|(basedirs::)?env_dir\('

declare -A allowed
while IFS= read -r line; do
    line="${line%%#*}"
    line="$(echo "$line" | sed 's/[[:space:]]*$//')"
    [ -n "$line" ] || continue
    allowed["$line"]=1
done < <(grep -vE '^\s*#' "$ALLOWLIST")

# Every allowlist line must carry a reason and point at a file that exists.
bad_allow=0
while IFS= read -r raw; do
    [ -n "$(echo "$raw" | sed 's/^[[:space:]]*//')" ] || continue
    case "$raw" in \#*) continue ;; esac
    path="${raw%%#*}"; path="$(echo "$path" | sed 's/[[:space:]]*$//')"
    reason="${raw#*#}"
    if [ "$raw" = "$path" ] || [ -z "$(echo "$reason" | tr -d '[:space:]')" ]; then
        echo "✗ $ALLOWLIST: '$path' has no reason (write: path  # reason)" >&2
        bad_allow=1
    fi
    if [ ! -f "$path" ]; then
        echo "✗ $ALLOWLIST: '$path' does not exist — remove the stale line" >&2
        bad_allow=1
    fi
done < "$ALLOWLIST"

status=$bad_allow
while IFS= read -r file; do
    if [ -z "${allowed[$file]:-}" ]; then
        status=1
        echo "✗ $file reads the process environment but is not in $ALLOWLIST:" >&2
        grep -nE "$PATTERN" "$file" | grep -vE '^\s*[0-9]+:\s*//' | sed 's/^/    /' >&2
    fi
done < <(list_sources | xargs -r grep -lE "$PATTERN" 2>/dev/null | sort -u)

# Allowlisted files that no longer read the environment are stale entries.
for file in "${!allowed[@]}"; do
    if ! grep -qE "$PATTERN" "$file"; then
        status=1
        echo "✗ $ALLOWLIST: $file no longer reads the environment — remove the line" >&2
    fi
done

if [ "$status" -ne 0 ]; then
    echo >&2
    echo "Read the value from the profile (extend Profile::validate if it is required)," >&2
    echo "delete the fallback, or — only for a sanctioned boot variable or platform" >&2
    echo "probe — add the file to $ALLOWLIST with its reason." >&2
    exit 1
fi
echo "✓ env readers are all allowlisted with reasons (${#allowed[@]} files)"
