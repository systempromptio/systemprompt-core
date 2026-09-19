#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
backup="$(mktemp -d)"
restore() {
    status=$?
    if [ -d "$backup/.sqlx" ]; then mv "$backup/.sqlx" .sqlx; fi
    rmdir "$backup"
    exit "$status"
}
trap restore EXIT
if [ -d .sqlx ]; then mv .sqlx "$backup/.sqlx"; fi
mapfile -t packages < <(cargo metadata --no-deps --format-version 1 --locked | jq -r '.packages[].name')
[ "${#packages[@]}" -gt 0 ] || { echo 'No workspace packages found' >&2; exit 1; }
args=()
for package in "${packages[@]}"; do args+=(-p "$package"); done
cargo clean "${args[@]}"
SQLX_OFFLINE=true cargo check --workspace --locked
