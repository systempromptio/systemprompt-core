#!/usr/bin/env bash
# Lifecycle state is compared as a closed enum, never as a string.
#
# A production comparison of a `status`, `state`, `kind` or `role` value
# against a string literal (`row.status == "failed"`,
# `x.kind.as_str() == "skill"`) fails: decode the value into its enum at the
# repository or wire edge and match on the variant. `crates/tests/` and the
# provider wire codecs in `crates/shared/wire/` (protocol JSON tags, not
# state) are out of scope.
#
#   scripts/lint-state-compare.sh [--paths P...]
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"
cd "$GATE_ROOT"

mapfile -t FILES < <(gate_files 'crates/**/*.rs' \
  | grep -v '^crates/tests/' | grep -v '^crates/shared/wire/' || true)
gate_pass_if_empty lint-state-compare "${#FILES[@]}"

PATTERN='(status|state|kind|role)(\.as_str\(\))? == "'
hits="$(grep -HnE "$PATTERN" "${FILES[@]}" || true)"
if [[ -n "$hits" ]]; then
  printf '%s\n' "$hits" | sed 's/^/lint-state-compare: /' >&2
  echo "lint-state-compare: FAILED (match the value's enum instead of comparing its text)" >&2
  exit 1
fi
echo "lint-state-compare: OK (no state field compared against a string)"
