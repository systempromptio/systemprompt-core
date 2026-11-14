#!/usr/bin/env bash
# Claude Code Stop hook (.claude/settings.json): before a session finishes it
# runs the cheap gate phases over what differs from the base,
#
#   scripts/gate.sh --changed --only fmt,gates --cost cheap
#
# so formatting and the text-scan source gates are judged before the push run
# is. Gates that compile (`--cost compile`, the rust-contracts scans) and the
# database phases never run here.
#
# Exit 0 lets the session stop: the gate passed, a tool it needs is missing
# (git, python3, cargo with rustfmt, just), the gate could not be planned (no
# base to diff against), or this stop is already the continuation the hook
# asked for (`stop_hook_active`). Exit 2 hands the failure summary to the
# session once, so it fixes the findings before stopping.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

input="$(cat || true)"
if printf '%s' "$input" | grep -qE '"stop_hook_active"[[:space:]]*:[[:space:]]*true'; then
    exit 0
fi
for tool in git python3 cargo just; do
    command -v "$tool" >/dev/null 2>&1 || { echo "stop-hook: $tool not found; gate skipped" >&2; exit 0; }
done
cargo fmt --version >/dev/null 2>&1 || { echo "stop-hook: rustfmt not installed; gate skipped" >&2; exit 0; }

status=0
output="$(bash scripts/gate.sh --changed --only fmt,gates --cost cheap 2>&1)" || status=$?
case "$status" in
    0) exit 0 ;;
    1) printf '%s\n' "$output" | tail -n 60 >&2; exit 2 ;;
    *) printf 'stop-hook: gate not planned, skipped:\n%s\n' "$(printf '%s\n' "$output" | tail -n 5)" >&2; exit 0 ;;
esac
