#!/usr/bin/env bash
# Production roots only. rust-contracts skips any directory literally named
# `tests` or `target`, so crates/tests and build output never enter the scan;
# pass explicit roots to narrow it (e.g. one crate while iterating).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$#" -eq 0 ]; then
    set -- "$root/crates/shared" "$root/crates/infra" "$root/crates/domain" \
           "$root/crates/app" "$root/crates/entry" "$root/systemprompt/src" \
           "$root/bin/bridge/src"
fi
cargo run --quiet --locked --manifest-path "$root/scripts/rust-contracts/Cargo.toml" -- discarded "$@"
