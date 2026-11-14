#!/usr/bin/env bash
# Gate: pages and jobs read facts, never re-derive them from the ledger.
#
# The ledger is the set of append-only raw tables the gateway, the hook plane,
# the tool plane and the governance spine write (`systemprompt_database::projection (crates/infra/database/src/projection/mod.rs)`).
# Exactly two things may read it: a projector in `schema/` (which turns rows
# into a stored fact, counter, projection or rollup) and a single-entity
# detail query (`WHERE <relation>.<id column> = $n`, no aggregate). A page or
# job that aggregates, groups, buckets or parses JSON out of a ledger table on
# every load re-computes an answer that was known once at write time; its
# cost grows with history and it is how a 7 s per-conversation refresh, a
# 715 ms per-user page and migration 111's timeout all happened.
#
# Flagged: any SQL string literal (or `.sql` file) under the infra, domain and
# app crates whose FROM/JOIN names a ledger relation AND that carries an
# aggregate (COUNT/SUM/AVG/MIN/MAX/percentile_*/*_agg/GROUP BY/date_trunc)
# or a JSON/regex operator (`->`, `->>`, `#>`, `@>`, `~`, `~*`, regexp_*).
# Exempt: a literal with no aggregate whose ledger read is pinned to one row
# by `<id column> = $n`.
#
# Each package's `gates/lint-ledger-reads.allow` lists its offenders that
# predate the gate as `<path>#<hash of the literal>` (path relative to the
# package). The list may only shrink: a line
# whose literal is gone or changed fails the gate (a touched offender is
# fixed, not re-baselined), and a new offender is never added to it.
#
# Core keeps documented raw-window analytics queries (percentiles, distinct
# counts, top-N substrings — not additive, so no rollup answers them) in the
# baseline deliberately; every other line is debt to burn down.
#
#   scripts/lint-ledger-reads.sh            # gate
#   scripts/lint-ledger-reads.sh --list     # print every current offender as a baseline line
#   scripts/lint-ledger-reads.sh [--list] --paths P...   # only files under P; the
#                                           # baseline is not judged for staleness
set -uo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"
cd "$GATE_ROOT"

MODE="${1:-}"

SCAN_DIRS=()
for d in crates/infra/*/src crates/domain/*/src crates/app/*/src; do
    [ -d "$d" ] && SCAN_DIRS+=("$d")
done
[ "${#SCAN_DIRS[@]}" -gt 0 ] || { echo "lint-ledger-reads: no scan directory found" >&2; exit 2; }
mapfile -t SCAN_DIRS < <(gate_roots "${SCAN_DIRS[@]}")
gate_pass_if_empty lint-ledger-reads "${#SCAN_DIRS[@]}"
FULL=0
gate_full_mode && FULL=1

MODE="$MODE" FULL="$FULL" python3 -B - "${SCAN_DIRS[@]}" <<'PY'
import hashlib, os, pathlib, re, sys
sys.path.insert(0, "scripts/lib")
import gate_scope
LEDGER = (
    "ai_requests", "ai_request_payloads", "ai_request_messages",
    "ai_request_tool_calls", "ai_request_client_evidence", "mcp_tool_executions",
    "mcp_artifacts", "agent_tasks", "task_messages", "logs", "analytics_events",
    "event_outbox", "user_sessions",
    # The report_* views are the ledger behind an archived-user anti-join.
    "report_agent_tasks",
    "report_ai_requests",
    "report_analytics_events",
    "report_bot_sessions",
    "report_clean_traffic",
    "report_engaged_traffic",
    "report_markdown_content",
    "report_mcp_tool_executions",
    "report_task_messages",
    "report_user_contexts",
    "report_user_sessions",
    "report_users",
)
LITERAL = re.compile(r'r(#*)"(.*?)"\1|"((?:[^"\\]|\\.)*)"', re.S)
READS = re.compile(r"\b(?:FROM|JOIN)\s+(?:ONLY\s+)?(" + "|".join(LEDGER) + r")\b", re.I)
AGG = re.compile(
    r"\b(?:COUNT|SUM|AVG|MIN|MAX|percentile_(?:cont|disc)|array_agg|string_agg|"
    r"jsonb?_agg|jsonb?_object_agg|bool_(?:or|and)|every|date_trunc|date_bin|width_bucket)\s*\("
    r"|\bGROUP\s+BY\b|\bCOUNT\s*\(\s*\*\s*\)",
    re.I,
)
JSON_OR_REGEX = re.compile(
    r"->>?|#>>?|@>|<@|\?\||\?&|\s~\*?\s|!~\*?\s|\bregexp_\w+\s*\(|\bSIMILAR\s+TO\b|\bjsonb?_(?:array_elements|each|path|extract)\w*\s*\(",
    re.I,
)
DETAIL = re.compile(r"\b(?:\w+\.)?(?:id|request_id|trace_id|context_id|event_id|execution_id|session_id|artifact_id)\s*=\s*\$\d+", re.I)
SQL_LIKE = re.compile(r"\b(?:SELECT|WITH|FROM)\b", re.I)

def normalise(sql):
    return re.sub(r"\s+", " ", sql).strip()

def digest(sql):
    return hashlib.sha1(normalise(sql).encode()).hexdigest()[:10]

def offends(sql):
    if not SQL_LIKE.search(sql):
        return None
    reads = sorted({m.group(1).lower() for m in READS.finditer(sql)})
    if not reads:
        return None
    aggregate = bool(AGG.search(sql))
    parses = bool(JSON_OR_REGEX.search(sql))
    if not aggregate and not parses:
        return None
    if not aggregate and DETAIL.search(sql):
        return None
    why = []
    if aggregate:
        why.append("aggregate")
    if parses:
        why.append("json/regex")
    return reads, why

def literals(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    if path.suffix == ".sql":
        yield 1, text
        return
    for m in LITERAL.finditer(text):
        sql = m.group(2) if m.group(2) is not None else m.group(3)
        yield text.count("\n", 0, m.start()) + 1, sql

found = {}
for root in (pathlib.Path(a) for a in sys.argv[1:]):
    for path in [root] if root.is_file() else sorted(root.rglob("*")):
        if path.suffix not in (".rs", ".sql") or not path.is_file():
            continue
        posix = path.as_posix()
        if "/schema/" in posix or "/tests/" in posix:
            continue
        for line, sql in literals(path):
            hit = offends(sql)
            if hit is None:
                continue
            key = f"{path.as_posix()}#{digest(sql)}"
            found[key] = (line, hit)

if os.environ.get("MODE") == "--list":
    for key in sorted(found):
        print(key)
    sys.exit(0)

try:
    baseline = set(gate_scope.allowlist("lint-ledger-reads"))
except gate_scope.AllowlistError as error:
    print(f"lint-ledger-reads: {error}", file=sys.stderr)
    sys.exit(2)

new = sorted(k for k in found if k not in baseline)
stale = sorted(k for k in baseline if k not in found) if os.environ.get("FULL") == "1" else []
if new:
    print("lint-ledger-reads: a page or job re-derives a fact from the ledger")
    for key in new:
        line, (reads, why) = found[key]
        path = key.split("#", 1)[0]
        print(f"  {path}:{line}: reads {', '.join(reads)} with {' + '.join(why)}")
    print("  Store the answer at write time (stamped column, counter, projection or rollup)")
    print("  and read that instead — systemprompt_database::projection (crates/infra/database/src/projection/mod.rs). The baseline only shrinks.")
if stale:
    print("lint-ledger-reads: gates/lint-ledger-reads.allow lists literals that no longer offend (or changed):")
    for key in stale:
        print(f"  {key}")
    print("  Delete those lines; a changed offender is fixed, never re-baselined.")
if new or stale:
    sys.exit(1)
print(f"lint-ledger-reads: OK ({len(found)} grandfathered)")
PY
