#!/usr/bin/env bash
# The bridge's IPC envelope types are exported to TypeScript under
# bin/bridge/bindings/ by a manual, `#[ignore]`d ts-rs test. Nothing compared
# the export against what was committed, so a changed variant shipped a stale
# binding silently. This regenerates into a scratch directory and diffs.
#
# Regenerate on purpose with `just bridge-bindings`.
set -euo pipefail

cd "$(dirname "$0")/.."

committed="bin/bridge/bindings/web/js/types"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/bridge-bindings.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

(
    cd crates/tests
    TS_RS_EXPORT_DIR="$scratch" cargo test -q -p systemprompt-bridge-ts-export-tests export_bindings -- --ignored >/dev/null
)

if diff -r "$scratch/web/js/types" "$committed"; then
    echo "check-bridge-bindings: OK ($committed matches the Rust types)"
else
    echo "check-bridge-bindings: $committed is stale -- run 'just bridge-bindings' and commit the result" >&2
    exit 1
fi
