#!/usr/bin/env bash
# The bridge is a standalone workspace, so it cannot inherit the root
# [workspace.lints] tables and carries a hand-mirrored copy. This gate fails
# when the two copies drift, which otherwise happens silently.
set -euo pipefail

cd "$(dirname "$0")/.."

extract() {
    awk -v s="$1" 'BEGIN{inb=0} $0=="["s"]"{inb=1;next} /^\[/{inb=0} inb && NF && !/^#/ {gsub(/ /,""); print}' "$2" | sort
}

status=0
for pair in "workspace.lints.rust:lints.rust" "workspace.lints.clippy:lints.clippy"; do
    root_table="${pair%%:*}"
    bridge_table="${pair##*:}"
    if ! diff <(extract "$root_table" Cargo.toml) <(extract "$bridge_table" bin/bridge/Cargo.toml) >/tmp/lints-sync-diff; then
        echo "lint-bridge-lints-sync: [$bridge_table] in bin/bridge/Cargo.toml has drifted from root [$root_table]:"
        cat /tmp/lints-sync-diff
        status=1
    fi
done
exit $status
