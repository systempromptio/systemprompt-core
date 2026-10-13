#!/usr/bin/env bash
# Tests run one process each under cargo-nextest against ONE shard database,
# so an instance id spelled as a literal is shared by every test process that
# spells it. Instance-scoped sweeps (`cleanup_orphaned_services`, the crashed /
# disabled / running sweeps, heartbeat reconciliation, the event outbox's
# own-origin filter) then act on another test's rows mid-test, and an
# in-process mutex cannot serialise across processes.
#
# Rule: in `crates/tests/**/*.rs`, a literal `InstanceId::new("…")` fails when
# the same file reaches an instance-scoped write path (services rows, instance
# registry, heartbeats, orphan cleanup, scheduler service state, the event
# outbox). Take a per-test id from `systemprompt_test_fixtures::unique_instance()`
# and bind it once where two rows in one test must share it.
#
# Escape a deliberate literal with `// lint-shared-test-ids: allow <reason>`
# on the same line.
#
#   scripts/lint-shared-test-ids.sh            # gate
#   scripts/lint-shared-test-ids.sh --count    # hit count only
set -uo pipefail
export LC_ALL=C
cd "$(dirname "$0")/.."

COUNT=0
[ "${1:-}" = "--count" ] && COUNT=1

WRITE_PATHS='ServiceRepository|register_instance|register_service|heartbeat[A-Za-z_]*\(|cleanup_orphaned|InstanceRegistry|(FROM|INTO|UPDATE)[[:space:]]+services([^_A-Za-z0-9]|$)|ServiceManagementService|ServiceStateVerifier|EventRouter::with_outbox|PostgresEventBridge|event_outbox|A2aDependencies|AgentOrchestrator|SchedulerRepository'

hits=""
while IFS= read -r file; do
    grep -qE "$WRITE_PATHS" "$file" || continue
    found=$(grep -nE 'InstanceId::new\("' "$file" | grep -v 'lint-shared-test-ids: allow' || true)
    [ -n "$found" ] || continue
    while IFS= read -r l; do
        hits+="$file:${l%%:*}: literal InstanceId in a file that writes instance-scoped rows"$'\n'
    done <<< "$found"
done < <(git ls-files 'crates/tests/*.rs' | sort)

if [ "$COUNT" -eq 1 ]; then printf '%s' "$hits" | grep -c .; exit 0; fi
if [ -n "$hits" ]; then
    echo "lint-shared-test-ids: a literal instance id is shared by every nextest process on the shard DB:" >&2
    printf '%s' "$hits" >&2
    echo "fix: use systemprompt_test_fixtures::unique_instance() (bind once per test where rows must share it)," >&2
    echo "     or annotate a deliberate literal with '// lint-shared-test-ids: allow <reason>'" >&2
    exit 1
fi
echo "lint-shared-test-ids: OK"
