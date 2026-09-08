#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$#" -eq 0 ]; then set -- "$root/bin/bridge/src"; fi
cargo run --quiet --locked --manifest-path "$root/scripts/rust-contracts/Cargo.toml" -- fail-open "$@"
