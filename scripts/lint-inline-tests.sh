#!/usr/bin/env bash
set -uo pipefail

# Machete rule: inline `#[cfg(test)] mod tests` is banned in production crates.
#
# Tests live in the separate `crates/tests` workspace, one crate per area, so
# they compile against the public surface a consumer actually has rather than
# reaching into private internals. An item a test needs is exposed with
# `#[doc(hidden)] pub`, which keeps it out of the published documentation while
# making the dependency on it explicit.
#
# `crates/tests/**` is exempt: those crates *are* the test workspace, and their
# `src/` modules carry the `#[cfg(test)]` that gates them.
#
# `bin/bridge` is its own cargo workspace, so a root `--workspace` invocation
# never reaches it. It is scanned explicitly here — four violations lived there
# undetected precisely because every gate stopped at the root manifest.

#   scripts/lint-inline-tests.sh            # core roots
#   scripts/lint-inline-tests.sh --paths P...  # only files under P
#   scripts/lint-inline-tests.sh ROOT...    # scan other source roots; paths
#                                           # resolve against the caller's cwd

. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"
if [ "$#" -eq 0 ]; then
    cd "$GATE_ROOT"
    mapfile -t roots < <(gate_roots crates bin systemprompt)
    gate_pass_if_empty lint-inline-tests "${#roots[@]}"
    set -- "${roots[@]}"
fi

fail=0
while IFS=: read -r file line _; do
    case "$file" in
        */tests/*|tests/*) continue ;;
    esac
    echo "$file:$line: inline \`#[cfg(test)] mod tests\` — move it to crates/tests/"
    fail=1
done < <(grep -rn --include='*.rs' -A1 '#\[cfg(test)\]' "$@" 2>/dev/null \
    | grep -B0 'mod tests' \
    | sed -E 's/-([0-9]+)-/:\1:/' \
    | grep -E '^[^:]+:[0-9]+:.*mod tests')

if [ "$fail" -eq 0 ]; then
    echo "lint-inline-tests: ok"
fi
exit "$fail"
