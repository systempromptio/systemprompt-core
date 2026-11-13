#!/usr/bin/env bash
# Runs every scripts/test-*.py to completion and exits 1 if any failed. A new
# script test is picked up by its file name; nothing registers it.
set -uo pipefail
cd "$(dirname "$0")/.."

failed=()
for test in scripts/test-*.py; do
    echo "═══ ${test} ═══"
    python3 "$test" || failed+=("$test")
done

if [ "${#failed[@]}" -gt 0 ]; then
    echo "✗ ${#failed[@]} script test(s) failed: ${failed[*]}" >&2
    exit 1
fi
echo "✓ all script tests passed"
