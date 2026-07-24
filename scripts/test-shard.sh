#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

# group → space-separated test-workspace path prefixes (anchored at /tests/ so
# the bridge `integration` crate is not double-matched by /integration/).
group_prefixes() {
  case "$1" in
    shared)      echo "/tests/unit/shared/" ;;
    infra)       echo "/tests/unit/infra/" ;;
    domain)      echo "/tests/unit/domain/" ;;
    app-runtime)   echo "/tests/unit/app/runtime/" ;;
    app-scheduler) echo "/tests/unit/app/scheduler/" ;;
    app-generator) echo "/tests/unit/app/generator/" ;;
    entry-api)     echo "/tests/unit/entry/api/" ;;
    entry-cli)     echo "/tests/unit/entry/cli/" ;;
    bridge)        echo "/tests/unit/bridge/" ;;
    integration)   echo "/tests/integration/" ;;
    edge)          echo "/tests/concurrency/ /tests/property/ /tests/contract/" ;;
    *) echo "unknown shard group: $1" >&2; exit 2 ;;
  esac
}
SHARD_GROUPS="shared infra domain app-runtime app-scheduler app-generator entry-api entry-cli bridge integration edge"

[ "${1:-}" = "--list" ] && { echo $SHARD_GROUPS; exit 0; }
group="${1:?usage: test-shard.sh <group|--list> [extra nextest args]}"; shift || true

prefixes="$(group_prefixes "$group")"
PKGS=$(cargo metadata --no-deps --format-version 1 --manifest-path crates/tests/Cargo.toml \
  | jq -r --arg ps "$prefixes" '
      ($ps | split(" ") | map(select(length > 0))) as $prefixes
      | .packages[] | .manifest_path as $m
      | select($prefixes | any(. as $p | $m | contains($p)))
      | "-p \(.name)"' | tr '\n' ' ')
test -n "$PKGS" || { echo "no packages matched group $group" >&2; exit 1; }
echo "shard $group: $PKGS"

# The entry-cli and integration shards spawn the real `systemprompt` binary;
# prebuild it once so subprocess fixtures never pay for (or time out on) a
# cold `cargo build` inside a running test.
case "$group" in
  entry-cli|integration)
    echo "==> Prebuilding systemprompt binary for subprocess tests"
    cargo build -p systemprompt-cli --bin systemprompt
    export SYSTEMPROMPT_BIN="$ROOT/target/debug/systemprompt"
    ;;
esac

cargo nextest run --profile "${NEXTEST_PROFILE:-default}" \
  --manifest-path crates/tests/Cargo.toml \
  --lib $PKGS --test-threads "${TEST_THREADS:-4}" "$@"
