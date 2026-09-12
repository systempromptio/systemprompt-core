#!/usr/bin/env bash
# Build (and, unless --no-run is given, run) the bridge test crates that the
# native quality matrix exercises. The package list lives in
# scripts/bridge-native-crates.txt so quality.yml and the local
# `just lint-bridge-native-tests` recipe share one source of truth.
#
# Usage: scripts/bridge-native-tests.sh [--no-run]
set -euo pipefail

cd "$(dirname "$0")/.."

mapfile -t crates < <(grep -vE '^\s*(#|$)' scripts/bridge-native-crates.txt)
if [ "${#crates[@]}" -eq 0 ]; then
    echo "scripts/bridge-native-crates.txt lists no crates" >&2
    exit 1
fi

args=()
for crate in "${crates[@]}"; do
    args+=(-p "$crate")
done

# These crates run with no database, so the offline cache is the only sqlx
# input allowed; a `query!` reachable from here is a defect, not a setup gap.
SQLX_OFFLINE=true exec cargo test --manifest-path crates/tests/Cargo.toml --locked "${args[@]}" "$@"
