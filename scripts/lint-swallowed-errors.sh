#!/usr/bin/env bash
# A `map_err(|_…| …)` on a network or database result (`.send()`, `.json()`,
# `.execute()`, `.fetch_*()` …) that never logs what it saw turns a provider's
# `403 unauthorized_client` into "service unavailable" with no trace. The
# closure names the error or logs it; `// Why: discard-ok: <reason>` on the
# line above is the only exemption. Production roots only.
#
#   scripts/lint-swallowed-errors.sh            # core roots
#   scripts/lint-swallowed-errors.sh ROOT...    # scan other source roots (absolute paths
#                                 # are fine; a directory named `tests` or
#                                 # `target` is skipped)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$#" -eq 0 ]; then
    set -- "$root/crates/shared" "$root/crates/infra" "$root/crates/domain" \
           "$root/crates/app" "$root/crates/entry" "$root/systemprompt/src" \
           "$root/bin/bridge/src"
fi
cargo run --quiet --locked --manifest-path "$root/scripts/rust-contracts/Cargo.toml" -- swallowed-errors "$@"
