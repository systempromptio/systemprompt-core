#!/usr/bin/env bash
# Build (and, unless --no-run is given, run) the test crates that a native
# quality matrix exercises. The package list is scripts/bridge-native-crates.txt
# (quality.yml's `bridge-native` job and `just lint-bridge-native-tests`), or
# the file `--list` names: scripts/device-native-crates.txt for the
# `device-native` job and `just lint-device-native-tests`. Workflow and local
# recipe read the same file, so the two cannot drift.
#
# Usage: scripts/bridge-native-tests.sh [--list FILE] [--no-run] [NEXTEST ARGS...]
set -euo pipefail

cd "$(dirname "$0")/.."

list=scripts/bridge-native-crates.txt
# Why: nextest rejects `--no-fail-fast` alongside `--no-run`.
run_args=(--no-fail-fast)
pass=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --list)
            list="${2:?--list needs a file}"
            shift
            ;;
        --no-run)
            run_args=()
            pass+=("$1")
            ;;
        *) pass+=("$1") ;;
    esac
    shift
done
[ -f "$list" ] || { echo "$list: no such crate list" >&2; exit 1; }

crates=()
while IFS= read -r crate; do
    crates+=("$crate")
done < <(grep -vE '^[[:space:]]*(#|$)' "$list")
if [ "${#crates[@]}" -eq 0 ]; then
    echo "$list lists no crates" >&2
    exit 1
fi

args=()
for crate in "${crates[@]}"; do
    args+=(-p "$crate")
done

# These crates run with no database, so the offline cache is the only sqlx
# input allowed; a `query!` reachable from here is a defect, not a setup gap.
# nextest runs each test in its own process, as every other run of this
# workspace does; the tests rely on that for process-wide state.
# Why: the macOS runner's bash is 3.2, where expanding an empty array under
# `set -u` is an unbound-variable error; `${a[@]+..}` expands to nothing instead.
SQLX_OFFLINE=true exec cargo nextest run --manifest-path crates/tests/Cargo.toml --locked \
    ${run_args[@]+"${run_args[@]}"} "${args[@]}" ${pass[@]+"${pass[@]}"}
