#!/usr/bin/env bash
# A tracing message is a constant; values travel as structured fields.
# `warn!("failed to run {cmd}")` is reported; `warn!(cmd = %cmd, "failed to
# run")` is the form. A bare `"{}"` / `"{name}"` message (a prepared freeform
# string) is exempt. Rule: rust-contracts `tracing-messages`.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$#" -eq 0 ]; then
    set -- "$root/crates/shared" "$root/crates/infra" "$root/crates/domain" \
           "$root/crates/app" "$root/crates/entry" "$root/systemprompt/src" \
           "$root/bin/bridge/src"
fi
cargo run --quiet --locked --manifest-path "$root/scripts/rust-contracts/Cargo.toml" -- tracing-messages "$@"
