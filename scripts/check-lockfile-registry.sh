#!/usr/bin/env bash
# Fails when a tracked Cargo.lock resolves a `systemprompt*` crate from anywhere
# but crates.io or the enclosing workspace.
#
# A `path+` or `git+` source in a committed lockfile is a local validation patch
# that escaped: it builds here and nowhere else, and a release cut from it
# publishes crates whose lockfile points at directories only this machine has.
#
# Only git-tracked lockfiles are walked -- an untracked one is invisible here,
# as it is to every other script gate.
set -euo pipefail

cd "$(dirname "$0")/.."

REGISTRY='registry+https://github.com/rust-lang/crates.io-index'

mapfile -t LOCKS < <(git ls-files '*Cargo.lock')
[ "${#LOCKS[@]}" -gt 0 ] || { echo "check-lockfile-registry: no tracked Cargo.lock files" >&2; exit 1; }

fail=0
for lock in "${LOCKS[@]}"; do
    findings=$(awk -v registry="$REGISTRY" '
function flush() {
    if (inpkg && name ~ /^systemprompt/ && source != "" && source != registry) {
        printf "  %s:%d\n    %s resolves from %s\n", FILENAME, start, name, source
    }
    inpkg = 0
}
/^\[\[package\]\]/ { flush(); name = ""; source = ""; start = FNR; inpkg = 1; next }
inpkg && /^name = / { name = $0; sub(/^name = "/, "", name); sub(/"$/, "", name) }
inpkg && /^source = / { source = $0; sub(/^source = "/, "", source); sub(/"$/, "", source) }
/^$/ { flush() }
END { flush() }
' "$lock")
    if [ -n "$findings" ]; then
        printf '%s\n' "$findings" >&2
        fail=1
    fi
done

if [ "$fail" -ne 0 ]; then
    echo "check-lockfile-registry: systemprompt crates must be workspace members (no source) or come from $REGISTRY" >&2
    echo "A path+ or git+ source means a local [patch.crates-io] was committed; drop it and re-resolve." >&2
    exit 1
fi

echo "check-lockfile-registry: all tracked lockfiles resolve systemprompt crates from the workspace or crates.io"
