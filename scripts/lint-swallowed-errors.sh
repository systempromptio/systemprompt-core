#!/usr/bin/env bash
# A `map_err(|_…| …)` on a network or database result (`.send()`, `.json()`,
# `.execute()`, `.fetch_*()` …) that never logs what it saw turns a provider's
# `403 unauthorized_client` into "service unavailable" with no trace. The
# closure names the error or logs it; `// Why: discard-ok: <reason>` on the
# line above is the only exemption. Production roots only.
#
#   scripts/lint-swallowed-errors.sh            # core roots
#   scripts/lint-swallowed-errors.sh --paths P...  # only files under P
#   scripts/lint-swallowed-errors.sh ROOT...    # scan other source roots (absolute paths
#                                 # are fine; a directory named `tests` or
#                                 # `target` is skipped)
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/rust-contracts.sh"
rust_contracts_gate swallowed-errors lint-swallowed-errors "$@"
