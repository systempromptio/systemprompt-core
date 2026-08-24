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

cd "$(dirname "$0")/.."

fail=0
while IFS=: read -r file line _; do
    case "$file" in
        crates/tests/*) continue ;;
    esac
    echo "$file:$line: inline \`#[cfg(test)] mod tests\` — move it to crates/tests/"
    fail=1
done < <(grep -rn --include='*.rs' -A1 '#\[cfg(test)\]' crates bin systemprompt 2>/dev/null \
    | grep -B0 'mod tests' \
    | sed -E 's/-([0-9]+)-/:\1:/' \
    | grep -E '^[^:]+:[0-9]+:.*mod tests')

if [ "$fail" -eq 0 ]; then
    echo "lint-inline-tests: ok"
fi
exit "$fail"
