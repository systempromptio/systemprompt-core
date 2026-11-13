#!/usr/bin/env bash
# A tracing message is a constant; values travel as structured fields.
# `warn!("failed to run {cmd}")` is reported; `warn!(cmd = %cmd, "failed to
# run")` is the form. A bare `"{}"` / `"{name}"` message (a prepared freeform
# string) is exempt. Rule: rust-contracts `tracing-messages`.
#
#   scripts/lint-tracing-messages.sh            # core roots
#   scripts/lint-tracing-messages.sh --paths P...  # only files under P
#   scripts/lint-tracing-messages.sh ROOT...    # scan other source roots (absolute paths
#                                 # are fine; a directory named `tests` or
#                                 # `target` is skipped)
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/rust-contracts.sh"
rust_contracts_gate tracing-messages lint-tracing-messages "$@"
