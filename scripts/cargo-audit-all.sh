#!/usr/bin/env bash
# Run cargo-audit across every workspace lockfile, sharing deny.toml's ignore list.
#
# cargo-deny's `advisories` check does not surface advisories the RustSec database
# marks `informational = "unsound"`, so a soundness hole in a dependency is invisible
# to `just deny`. cargo-audit does surface them, and `--deny unsound` makes one fail.
#
# The ignore list is generated from deny.toml on every run rather than duplicated in
# an audit.toml, so there is exactly one place where a suppression is justified.
set -euo pipefail

cd "$(dirname "$0")/.."

DENY_CONFIG="deny.toml"
[[ -f "$DENY_CONFIG" ]] || { echo "deny.toml not found" >&2; exit 1; }

mapfile -t IGNORED < <(grep -oE 'id = "RUSTSEC-[0-9]{4}-[0-9]{4}"' "$DENY_CONFIG" | grep -oE 'RUSTSEC-[0-9]{4}-[0-9]{4}')
if [[ ${#IGNORED[@]} -eq 0 ]]; then
    echo "no advisory ignores parsed from deny.toml — refusing to run with an empty list" >&2
    exit 1
fi

IGNORE_ARGS=()
for id in "${IGNORED[@]}"; do
    IGNORE_ARGS+=(--ignore "$id")
done

echo "==> sharing ${#IGNORED[@]} advisory ignores from deny.toml"

WORKSPACES=("$@")
if [[ ${#WORKSPACES[@]} -eq 0 ]]; then
    echo "usage: $0 <workspace-dir>..." >&2
    exit 1
fi

status=0
for w in "${WORKSPACES[@]}"; do
    lock="${w%/}/Cargo.lock"
    if [[ ! -f "$lock" ]]; then
        echo "==> cargo audit: ${w} (skipped, no Cargo.lock)"
        continue
    fi
    echo "==> cargo audit: ${w}"
    cargo audit --file "$lock" --deny unsound "${IGNORE_ARGS[@]}" || status=1
done

exit "$status"
