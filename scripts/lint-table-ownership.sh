#!/usr/bin/env bash
# Table ownership is a layer boundary: infra never queries a domain table and
# a domain never queries another domain's table (cross-domain data flows
# through a shared-layer trait, wired at a composition root). `lint-layers`
# walks the Cargo graph and cannot see SQL, which is how `infra/logging` came
# to compile `sqlx::query!` over `agent_tasks` with no Cargo edge at all.
#
# For every `sqlx::query*!(` / `sqlx::query*(` whose SQL is a string literal in
# `crates/infra/**` and `crates/domain/<crate>/**`, every table named after
# FROM / JOIN / INTO / UPDATE / DELETE FROM must be declared by a
# `CREATE TABLE` or `CREATE VIEW` in that crate's `schema/*.sql`; an infra
# crate may also read any other infra crate's tables. CTE names, subquery
# aliases, set-returning functions and the Postgres catalogs
# (`pg_*`, `information_schema`, `pg_catalog`) are not tables and are ignored.
#
#   scripts/lint-table-ownership.sh            # gate; offenders grouped by crate
#   scripts/lint-table-ownership.sh --count    # hit count only
set -uo pipefail
cd "$(dirname "$0")/.."
command -v python3 >/dev/null || { echo "lint-table-ownership: python3 required" >&2; exit 2; }

COUNT=0
[ "${1:-}" = "--count" ] && COUNT=1

git ls-files -co --exclude-standard 'crates/infra/*/src/**/*.rs' 'crates/infra/*/src/*.rs' \
    'crates/domain/*/src/**/*.rs' 'crates/domain/*/src/*.rs' \
    'crates/infra/*/schema/*.sql' 'crates/domain/*/schema/*.sql' \
  | sort -u | COUNT="$COUNT" python3 -c '
import os, re, sys
from collections import defaultdict

files = [line.strip() for line in sys.stdin if line.strip()]
count_only = os.environ.get("COUNT") == "1"

def crate_of(path):
    parts = path.split("/")
    return "/".join(parts[:3])

DECL = re.compile(r"\bCREATE\s+(?:OR\s+REPLACE\s+)?(?:UNLOGGED\s+|TEMP(?:ORARY)?\s+|MATERIALIZED\s+)?(?:TABLE|VIEW)\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:[a-z_][a-z0-9_]*\.)?([a-z_][a-z0-9_]*)", re.I)
declared = defaultdict(set)
for path in files:
    if path.endswith(".sql"):
        text = open(path, encoding="utf-8", errors="replace").read()
        for name in DECL.findall(text):
            declared[crate_of(path)].add(name.lower())
infra_tables = set()
for crate, names in declared.items():
    if crate.startswith("crates/infra/"):
        infra_tables |= names

MACRO = re.compile(r"\bsqlx::query(?:_as|_scalar|_as_unchecked|_scalar_unchecked|_unchecked)?!?\s*\(")
IDENT_ARG = re.compile(r"\s*(?:[A-Za-z_][A-Za-z0-9_:<>]*)\s*,\s*")

def literal_at(src, i):
    """Return (sql, end) for a string literal starting at src[i], else None."""
    m = re.match(r"r(#*)\"", src[i:])
    if m:
        hashes = m.group(1)
        end = src.find("\"" + hashes, i + len(m.group(0)))
        if end < 0:
            return None
        return src[i + len(m.group(0)):end], end + 1 + len(hashes)
    if src[i] != "\"":
        return None
    j = i + 1
    out = []
    while j < len(src):
        c = src[j]
        if c == "\\":
            nxt = src[j + 1] if j + 1 < len(src) else ""
            if nxt == "\n":
                j += 2
                while j < len(src) and src[j] in " \t\r\n":
                    j += 1
                continue
            out.append(" " if nxt in "nrt" else nxt)
            j += 2
            continue
        if c == "\"":
            return "".join(out), j + 1
        out.append(c)
        j += 1
    return None

CTE = re.compile(r"(?:\bWITH\s+(?:RECURSIVE\s+)?|,\s*)([a-z_][a-z0-9_]*)\s+AS\s*(?:(?:NOT\s+)?MATERIALIZED\s*)?\(", re.I)
REF = re.compile(r"(?<!DISTINCT\s)\b(FROM|JOIN|INTO|UPDATE)\s+(?:ONLY\s+)?(?:([a-z_][a-z0-9_]*)\.)?([a-z_][a-z0-9_]*)\b(?!\s*\()", re.I)
SYSTEM_SCHEMAS = {"information_schema", "pg_catalog"}
SCALAR_FROM = re.compile(r"\b(?:EXTRACT|SUBSTRING|TRIM|OVERLAY)\s*\([^()]*\)", re.I)
NOT_TABLES = {"set", "only", "lateral", "skip", "nowait", "select", "values", "of", "where", "returning"}

def tables_in(sql):
    sql = SCALAR_FROM.sub(" ", sql)
    ctes = {name.lower() for name in CTE.findall(sql)}
    found = set()
    for keyword, schema, name in REF.findall(sql):
        name = name.lower()
        if name in NOT_TABLES or name in ctes:
            continue
        if schema.lower() in SYSTEM_SCHEMAS or name.startswith("pg_"):
            continue
        found.add(name)
    return found

offenders = defaultdict(list)
scanned = 0
for path in files:
    if not path.endswith(".rs"):
        continue
    crate = crate_of(path)
    allowed = declared.get(crate, set()) | (infra_tables if crate.startswith("crates/infra/") else set())
    src = open(path, encoding="utf-8", errors="replace").read()
    for m in MACRO.finditer(src):
        i = m.end()
        while i < len(src) and src[i] in " \t\r\n":
            i += 1
        arg = IDENT_ARG.match(src, i)
        if arg and literal_at(src, arg.end()) is not None:
            i = arg.end()
        lit = literal_at(src, i)
        if lit is None:
            continue
        scanned += 1
        sql, _ = lit
        line = src.count("\n", 0, m.start()) + 1
        for table in sorted(tables_in(sql)):
            if table not in allowed:
                owner = sorted(c for c, names in declared.items() if table in names)
                joined = ", ".join(owner)
                where = f"owned by {joined}" if owner else "declared by no schema/*.sql"
                offenders[crate].append(f"  {path}:{line}: {table} ({where})")

total = sum(len(v) for v in offenders.values())
if count_only:
    print(total)
    sys.exit(0)
if scanned == 0:
    print("lint-table-ownership: no sqlx query literals scanned — scope broken?", file=sys.stderr)
    sys.exit(1)
if total:
    print("lint-table-ownership: a crate may only query tables its own schema/*.sql declares (infra may read infra):", file=sys.stderr)
    for crate in sorted(offenders):
        print(f"{crate} ({len(offenders[crate])}):", file=sys.stderr)
        for line in offenders[crate]:
            print(line, file=sys.stderr)
    sys.exit(1)
print(f"lint-table-ownership: OK ({scanned} query literals, {len(declared)} schema crates)")
'
