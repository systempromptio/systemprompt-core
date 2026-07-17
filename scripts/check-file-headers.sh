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
while IFS= read -r file; do
  first=$(head -1 "$file")
  case "$first" in
    '//! '* | '#!['*) ;;
    *)
      echo "MISSING DOC HEAD: $file"
      fail=1
      continue
      ;;
  esac
  if [[ "$first" == "$LICENSE_1" ]]; then
    echo "LICENSE BEFORE PURPOSE LINE: $file"
    fail=1
    continue
  fi
  if ! grep -qFx "$LICENSE_1" "$file" || ! grep -qFx "$LICENSE_2" "$file"; then
    echo "MISSING LICENSE REFERENCE: $file"
    fail=1
  fi
done < <(find "${ROOTS[@]}" -name '*.rs' -not -path '*/tests/*' ! -name build.rs)

if [[ "$fail" -ne 0 ]]; then
  echo "check-file-headers: FAILED"
  exit 1
fi
echo "check-file-headers: OK"
