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
    # `integration` was one shard of 18 packages and 1684 tests, and at ~21min
    # it was CI's entire critical path while no other shard passed 5min. Split
    # three ways. `cli` and `cloud` stay together: both are pinned to single
    # nextest test-groups (cli-cloud-harness, cloud-checkout-port), so splitting
    # them apart would buy no parallelism.
    integration-api)  echo "/tests/integration/api/" ;;
    integration-cli)  echo "/tests/integration/cli/ /tests/integration/cloud/" ;;
    integration-rest) echo "/tests/integration/agent/ /tests/integration/analytics/ \
                             /tests/integration/content/ /tests/integration/database/ \
                             /tests/integration/events/ /tests/integration/extension/ \
                             /tests/integration/files/ /tests/integration/gateway/ \
                             /tests/integration/generator/ /tests/integration/mcp/ \
                             /tests/integration/oauth/ /tests/integration/runtime/ \
                             /tests/integration/security/ /tests/integration/scheduler/ \
                             /tests/integration/users/" ;;
    edge)          echo "/tests/concurrency/ /tests/property/ /tests/contract/" ;;
    *) echo "unknown shard group: $1" >&2; exit 2 ;;
  esac
}
SHARD_GROUPS="shared infra domain app-runtime app-scheduler app-generator entry-api entry-cli bridge integration-api integration-cli integration-rest edge"

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
  entry-cli|integration-api|integration-cli|integration-rest)
    echo "==> Prebuilding systemprompt binary for subprocess tests"
    cargo build -p systemprompt-cli --bin systemprompt
    export SYSTEMPROMPT_BIN="$ROOT/target/debug/systemprompt"
    ;;
esac

# Scale to the host but keep the old ceiling: each test opens its own pool of up
# to 8 connections (crates/tests/common/fixtures/src/db.rs) against a server
# whose max_connections is 100, so parallelism above 8 exhausts the budget.
cores="$(nproc 2>/dev/null || echo 4)"
threads="${TEST_THREADS:-$(( cores < 8 ? cores : 8 ))}"

cargo nextest run --profile "${NEXTEST_PROFILE:-default}" \
  --manifest-path crates/tests/Cargo.toml \
  --lib $PKGS --test-threads "$threads" "$@"
