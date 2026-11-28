#!/usr/bin/env python3
"""Count the per-case rows of every migrated test batch, and check each file's totals line.

Reads every `cases.md` under `crates/tests/migrated/*/` and, with `--template DIR`,
every `cases.md` under `DIR/extensions/harness/tests/migrated/`. A case row is a row
of a table whose header names an `id`, a `class` (or `disposition`) and a `state`
column; columns are found by header name, never by position. `--check` exits 1 when a
row is `open`, `written` or `manual`, or when a file's own "Cases:" or "Totals:" line
disagrees with its rows.
"""
import argparse
from collections import Counter
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
COLUMNS = ("cases", "passing", "ledger", "covered", "not applicable", "deferred",
           "open", "written", "manual", "tour")
UNFINISHED = ("open", "written", "manual")
CLASS_BUCKETS = {"covered": "covered", "not-applicable": "not applicable", "defer": "deferred"}
STATE_BUCKETS = {"passing", "ledger", "open", "written", "manual", "tour"}
TOTAL_NAMES = {
    "passing": "passing", "ported": "passing", "ledger": "ledger", "difference": "ledger",
    "covered": "covered", "not applicable": "not applicable", "not-applicable": "not applicable",
    "n/a": "not applicable", "deferred": "deferred", "defer": "deferred", "open": "open",
    "written": "written", "manual": "manual", "tour": "tour",
}
PAIR = re.compile(r"(not applicable|not-applicable|n/a|[a-z]+) (\d+)")


def split_row(line):
    cells = re.split(r"(?<!\\)\|", line.strip())
    return [cell.strip() for cell in cells[1:-1]]


def is_rule(cells):
    return all(set(cell) <= set("-: ") for cell in cells)


def case_rows(text):
    header = None
    for line in text.splitlines():
        if not line.startswith("|"):
            header = None
            continue
        cells = split_row(line)
        if header is None:
            header = cells
            continue
        if is_rule(cells):
            continue
        kind = "class" if "class" in header else "disposition" if "disposition" in header else None
        if "id" not in header or "state" not in header or kind is None:
            continue
        if len(cells) != len(header):
            raise ValueError(f"row {cells[0]!r} has {len(cells)} cells, header has {len(header)}")
        yield cells[header.index(kind)], cells[header.index("state")]


def bucket(kind, state):
    if kind in CLASS_BUCKETS:
        return CLASS_BUCKETS[kind]
    if state in STATE_BUCKETS:
        return state
    raise ValueError(f"unknown class {kind!r} with state {state!r}")


def count(text):
    counts = Counter({column: 0 for column in COLUMNS})
    for kind, state in case_rows(text):
        counts["cases"] += 1
        counts[bucket(kind, state)] += 1
    return counts


def declared_totals(text):
    """The counts a file's own totals line states, or None when it has none."""
    for line in text.splitlines():
        if line.startswith("Totals:"):
            head, _, tail = line[len("Totals:"):].partition(";")
            total = re.search(r"(\d+) case rows", tail)
        elif line.startswith("Cases:"):
            parts = line[len("Cases:"):].split(".")
            head = parts[1] if len(parts) > 1 else ""
            total = re.match(r"\s*(\d+)", parts[0])
        else:
            continue
        declared = {}
        for name, value in PAIR.findall(head):
            if name in TOTAL_NAMES:
                declared[TOTAL_NAMES[name]] = declared.get(TOTAL_NAMES[name], 0) + int(value)
        if total:
            declared["cases"] = int(total.group(1))
        return declared
    return None


def disagreements(counts, declared):
    if declared is None:
        return ["no Cases: or Totals: line"]
    found = []
    for column in COLUMNS:
        stated = declared.get(column)
        if stated is None and counts[column] == 0:
            continue
        if stated != counts[column]:
            found.append(f"{column}: line says {stated if stated is not None else 'nothing'}, rows say {counts[column]}")
    return found


def batches(template):
    files = sorted((ROOT / "crates" / "tests" / "migrated").glob("*/cases.md"))
    found = [(f"core/{path.parent.name}", path) for path in files]
    if template:
        base = Path(template) / "extensions" / "harness" / "tests" / "migrated"
        for path in sorted(base.glob("**/cases.md")):
            found.append(("template/" + path.parent.relative_to(base).as_posix(), path))
    return found


def render(table):
    headers = ("batch",) + COLUMNS
    rows = [headers] + [(name,) + tuple(str(counts[c]) for c in COLUMNS) for name, counts in table]
    widths = [max(len(row[i]) for row in rows) for i in range(len(headers))]
    lines = ["  ".join(cell.ljust(widths[i]) if i == 0 else cell.rjust(widths[i])
                       for i, cell in enumerate(row)) for row in rows]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--template", metavar="DIR", help="also count the template checkout at DIR")
    parser.add_argument("--check", action="store_true", help="exit 1 on unfinished rows or wrong totals lines")
    args = parser.parse_args()
    table = []
    problems = []
    total = Counter({column: 0 for column in COLUMNS})
    for name, path in batches(args.template):
        text = path.read_text(encoding="utf-8")
        try:
            counts = count(text)
        except ValueError as error:
            problems.append(f"{path}: {error}")
            continue
        table.append((name, counts))
        total.update(counts)
        for problem in disagreements(counts, declared_totals(text)):
            problems.append(f"{path}: totals line disagrees: {problem}")
        for state in UNFINISHED:
            if counts[state]:
                problems.append(f"{path}: {counts[state]} {state} rows")
    table.append(("total", total))
    print(render(table))
    if args.check and problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
