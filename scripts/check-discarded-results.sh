#!/usr/bin/env bash
# Production roots only. rust-contracts skips any directory literally named
# `tests` or `target`, so crates/tests and build output never enter the scan;
# `--paths P...` narrows it to part of the tree (e.g. one crate while
# iterating); explicit ROOTs scan other trees.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/rust-contracts.sh"
rust_contracts_gate discarded lint-discarded-results "$@"
