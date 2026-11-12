#!/usr/bin/env bash
# Verify every production Rust source file carries a rustdoc module head that
# opens with a purpose line and ends with the exact BSL-1.1 license reference.
set -euo pipefail

cd "$(dirname "$0")/.."

LICENSE_1='//! Copyright (c) systemprompt.io — Business Source License 1.1.'
LICENSE_2='//! See <https://systemprompt.io> for licensing details.'

ROOTS=(
  crates/shared crates/infra crates/domain crates/app crates/entry
  systemprompt/src bin/bridge/src
)

fail=0
find "${ROOTS[@]}" -name '*.rs' -not -path '*/tests/*' ! -name build.rs \
  | awk -v L1="$LICENSE_1" -v L2="$LICENSE_2" '
      {
        file = $0
        first = ""; has1 = 0; has2 = 0; n = 0
        while ((getline line < file) > 0) {
          if (++n == 1) first = line
          if (line == L1) has1 = 1
          if (line == L2) has2 = 1
        }
        close(file)
        if (first !~ /^\/\/! / && first !~ /^#!\[/) { print "MISSING DOC HEAD: " file; bad = 1; next }
        if (first == L1) { print "LICENSE BEFORE PURPOSE LINE: " file; bad = 1; next }
        if (!has1 || !has2) { print "MISSING LICENSE REFERENCE: " file; bad = 1 }
      }
      END { exit bad }
    ' || fail=1

if [[ "$fail" -ne 0 ]]; then
  echo "check-file-headers: FAILED"
  exit 1
fi
echo "check-file-headers: OK"
