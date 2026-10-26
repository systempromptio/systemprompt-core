#!/usr/bin/env bash
# Pre-merge gate for migration files that must be safe on a large, live
# database: a migration runs at boot, in one transaction per file, holding
# every lock it takes until the file commits, while other nodes keep writing.
#
# Scans the migration files ADDED since the last release tag (a migration
# already on `next` is immutable, so earlier files are never re-judged) and
# fails on:
#
#   constraint   ALTER TABLE … ADD CONSTRAINT … CHECK | FOREIGN KEY without
#                NOT VALID in the statement, or NOT VALID without a later
#                VALIDATE CONSTRAINT of the same name in the file.
#   index        CREATE [UNIQUE] INDEX on a table in scripts/large-tables.txt
#                without CONCURRENTLY, unless a `-- Why:` line sits directly
#                above the statement; CONCURRENTLY in a file whose first line
#                is not `-- @no-transaction` (it cannot run in a transaction).
#   bulk-write   a top-level UPDATE or DELETE on a large table with no
#                `EXISTS (` / `IF EXISTS` guard or LIMIT in the statement and
#                no `-- Why: bounded:` line directly above.
#   not-null     ALTER TABLE <large> ADD COLUMN … NOT NULL without DEFAULT.
#   inline       a `LANGUAGE plpgsql` function the migration defines that a
#                CREATE [OR REPLACE] VIEW in the same crate's schema calls
#                (the planner inlines `LANGUAGE sql` only, so partial indexes
#                stop matching the view's predicate).
#
# Statements are split on `;` outside dollar-quoted bodies; a body's own
# UPDATE/DELETE is the function's behaviour, not a bulk write.
#
# Usage: lint-migrations.sh [--large-tables FILE] [ROOT]
#   With ROOT, every `**/schema/migrations/*.sql` under it is scanned (the
#   fixture form). Without it, the files `git diff --diff-filter=A` lists
#   between the last `v*` tag and HEAD.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LARGE_TABLES="$ROOT/scripts/large-tables.txt"
SCAN_ROOT=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --large-tables) LARGE_TABLES="${2:?--large-tables needs a file}"; shift 2 ;;
        *) SCAN_ROOT="$1"; shift ;;
    esac
done

if [ -n "$SCAN_ROOT" ]; then
    mapfile -t files < <(find "$SCAN_ROOT" -type f -path '*/schema/migrations/*.sql' -not -path '*/target/*' | sort)
else
    # Why: CI checkouts are shallow and tagless; the release tag is fetched on
    # demand so the "added since the last release" window is the same everywhere.
    if ! tag="$(git -C "$ROOT" describe --tags --abbrev=0 --match 'v*' 2>/dev/null)"; then
        git -C "$ROOT" fetch --quiet --tags --filter=blob:none origin >/dev/null 2>&1 || true
        tag="$(git -C "$ROOT" describe --tags --abbrev=0 --match 'v*' 2>/dev/null)" || {
            echo "lint-migrations: no v* release tag reachable from HEAD (fetch tags first)" >&2
            exit 2
        }
    fi
    mapfile -t files < <(git -C "$ROOT" diff --name-only --diff-filter=A "$tag"..HEAD -- \
        'crates/*/*/schema/migrations/*.sql' | sed "s#^#$ROOT/#" | sort)
fi

if [ "${#files[@]}" -eq 0 ]; then
    echo "lint-migrations: OK (no migrations added since the last release)"
    exit 0
fi

python3 - "$LARGE_TABLES" "${files[@]}" <<'PY'
import re
import sys
from pathlib import Path

large_tables_path, *files = sys.argv[1:]
large = {
    line.split("#", 1)[0].strip()
    for line in Path(large_tables_path).read_text().splitlines()
    if line.split("#", 1)[0].strip()
}

IDENT = r'"?([A-Za-z_][A-Za-z0-9_]*)"?'


def statements(text):
    """Yield (first_line_number, statement_text) split on `;` outside $$ bodies."""
    out, start, depth_tag, line = [], 1, None, 1
    i = 0
    while i < len(text):
        if text[i] == "\n":
            line += 1
        if depth_tag is None:
            m = re.match(r"\$[A-Za-z_]*\$", text[i:])
            if m:
                depth_tag = m.group(0)
                out.append(m.group(0))
                i += len(m.group(0))
                continue
            if text[i] == ";":
                yield start, "".join(out)
                out, start = [], line
                i += 1
                continue
        elif text.startswith(depth_tag, i):
            out.append(depth_tag)
            i += len(depth_tag)
            depth_tag = None
            continue
        out.append(text[i])
        i += 1
    if "".join(out).strip():
        yield start, "".join(out)


def strip_comments(stmt):
    return re.sub(r"--[^\n]*", "", stmt)


def leading_comment(stmt):
    return [l.strip() for l in stmt.splitlines() if l.strip().startswith("--")]


def body_without_dollar(stmt):
    return re.sub(r"(\$[A-Za-z_]*\$).*?\1", "$$", stmt, flags=re.S)


def check_file(path):
    text = Path(path).read_text()
    rel = path
    problems = []
    first = next((l.strip() for l in text.splitlines() if l.strip()), "")
    no_transaction = first == "-- @no-transaction"
    validated = {
        m.group(1).lower()
        for m in re.finditer(r"VALIDATE\s+CONSTRAINT\s+" + IDENT, text, re.I)
    }
    plpgsql_functions = set()
    for line_no, raw in statements(text):
        comments = leading_comment(raw)
        why = any(c.startswith("-- Why:") for c in comments)
        why_bounded = any(c.startswith("-- Why: bounded:") for c in comments)
        stmt = strip_comments(raw).strip()
        flat = " ".join(stmt.split())
        if not flat:
            continue
        head = body_without_dollar(flat)

        m = re.match(r"ALTER\s+TABLE\s+(?:ONLY\s+)?" + IDENT + r"\s+ADD\s+CONSTRAINT\s+" + IDENT + r"\s+(CHECK|FOREIGN\s+KEY)\b", head, re.I)
        if m:
            name = m.group(2).lower()
            if not re.search(r"\bNOT\s+VALID\b", head, re.I):
                problems.append((line_no, "constraint", f"ADD CONSTRAINT {name} without NOT VALID (add NOT VALID, then VALIDATE CONSTRAINT {name})"))
            elif name not in validated:
                problems.append((line_no, "constraint", f"ADD CONSTRAINT {name} NOT VALID is never validated in this file"))

        m = re.match(r"CREATE\s+(UNIQUE\s+)?INDEX\s+(CONCURRENTLY\s+)?(?:IF\s+NOT\s+EXISTS\s+)?" + IDENT + r"\s+ON\s+(?:ONLY\s+)?" + IDENT, head, re.I)
        if m:
            concurrently, table = bool(m.group(2)), m.group(4).lower()
            if concurrently and not no_transaction:
                problems.append((line_no, "index", f"CREATE INDEX CONCURRENTLY on {table} needs `-- @no-transaction` as the file's first line"))
            if not concurrently and table in large and not why:
                problems.append((line_no, "index", f"CREATE INDEX on large table {table} without CONCURRENTLY (build it CONCURRENTLY in a `-- @no-transaction` file, or justify with a `-- Why:` line directly above)"))
        m = re.match(r"DROP\s+INDEX\s+CONCURRENTLY\b", head, re.I)
        if m and not no_transaction:
            problems.append((line_no, "index", "DROP INDEX CONCURRENTLY needs `-- @no-transaction` as the file's first line"))

        m = re.match(r"(UPDATE|DELETE\s+FROM)\s+(?:ONLY\s+)?" + IDENT + r"\b", head, re.I)
        if m:
            table = m.group(2).lower()
            guarded = re.search(r"\bEXISTS\s*\(|\bIF\s+EXISTS\b|\bLIMIT\s+\d+", head, re.I)
            if table in large and not guarded and not why_bounded:
                problems.append((line_no, "bulk-write", f"unbounded {m.group(1).split()[0].upper()} on large table {table} (guard with EXISTS (…) or a LIMIT, or justify with `-- Why: bounded:` directly above)"))

        m = re.match(r"ALTER\s+TABLE\s+(?:ONLY\s+)?" + IDENT + r"\s+ADD\s+(?:COLUMN\s+)?(?:IF\s+NOT\s+EXISTS\s+)?" + IDENT + r"\s+(.*)$", head, re.I)
        if m and m.group(1).lower() in large:
            column, rest = m.group(2), m.group(3)
            if re.search(r"\bNOT\s+NULL\b", rest, re.I) and not re.search(r"\bDEFAULT\b", rest, re.I):
                problems.append((line_no, "not-null", f"ADD COLUMN {column} NOT NULL on large table {m.group(1).lower()} without a DEFAULT (rewrites every row under ACCESS EXCLUSIVE)"))

        m = re.match(r"CREATE\s+(?:OR\s+REPLACE\s+)?FUNCTION\s+" + IDENT, head, re.I)
        if m and re.search(r"\bLANGUAGE\s+plpgsql\b", head, re.I):
            plpgsql_functions.add((line_no, m.group(1).lower()))

    if plpgsql_functions:
        schema_dir = Path(path).resolve().parent.parent
        view_calls = set()
        for sql in schema_dir.rglob("*.sql"):
            body = strip_comments(sql.read_text())
            for view in re.finditer(r"CREATE\s+(?:OR\s+REPLACE\s+)?VIEW\b(.*?);", body, re.I | re.S):
                view_calls |= {c.lower() for c in re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", view.group(1))}
        for line_no, name in sorted(plpgsql_functions):
            if name in view_calls:
                problems.append((line_no, "inline", f"function {name} is LANGUAGE plpgsql but a view in this schema calls it; make it LANGUAGE sql so the planner inlines it"))
    return rel, problems


total = 0
for path in files:
    rel, problems = check_file(path)
    for line_no, rule, message in problems:
        total += 1
        print(f"{rel}:{line_no}: [{rule}] {message}")

if total:
    print(f"lint-migrations: {total} violation(s) in migrations added since the last release", file=sys.stderr)
    sys.exit(1)
print(f"lint-migrations: OK ({len(files)} migration(s) scanned)")
PY
