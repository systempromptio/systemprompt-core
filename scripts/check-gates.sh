#!/usr/bin/env bash
# Runs every source gate in scripts/check-gates.txt to completion, then the
# rust-contracts tests, and prints a PASS/FAIL summary. Exits 1 if any failed.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

results=()
failed=()

run_gate() {
    local name="$1"; shift
    echo "═══ ${name} ═══"
    if "$@"; then
        results+=("PASS  ${name}")
    else
        results+=("FAIL  ${name}")
        failed+=("${name}")
    fi
}

while IFS= read -r gate; do
    [ -n "$gate" ] || continue
    run_gate "$gate" just "$gate"
done < scripts/check-gates.txt

run_gate rust-contracts cargo test --locked --manifest-path scripts/rust-contracts/Cargo.toml

echo
echo "═══ check-gates summary ═══"
printf '%s\n' "${results[@]}"
if [ "${#failed[@]}" -gt 0 ]; then
    echo
    echo "✗ ${#failed[@]} gate(s) failed: ${failed[*]}" >&2
    exit 1
fi
echo "✓ all ${#results[@]} gates passed"
