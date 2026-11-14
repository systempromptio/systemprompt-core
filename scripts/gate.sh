#!/usr/bin/env bash
# The one gate entry point, the same locally and in CI:
#
#   scripts/gate.sh --unit CRATE...          these crates and their reverse closure
#   scripts/gate.sh --changed [--base SHA]   what differs from the base, working tree included
#   scripts/gate.sh --full                   everything (`just check-gates` is --full --only gates)
#
#   --only fmt,clippy,doc,gates,supply,scripts,tests   run just these phases
#   --skip tests,doc                                   leave these out
#   --group sql|errors|bridge|release                  gates of one quality.yml source-gates job
#   --cost cheap[,compile,db]                          gates of these scripts/gate-scopes.txt costs
#   --dry-run                                          print phases and exact commands, run nothing
#   --selection FILE                                   use a precomputed affected.py JSON as the scope
#
# Every phase runs to completion and a summary names each failure; the exit
# status is 1 if any step failed. One JSON line per run is appended to
# target/gate-log/runs.jsonl (GATE_LOG overrides). The plan and the runner live
# in scripts/lib/gate_runner.py.
set -euo pipefail
exec python3 -B "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/gate_runner.py" "$@"
