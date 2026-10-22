#!/usr/bin/env bash
# Each persisted vocabulary registered in scripts/sql-vocabularies.txt has one
# Rust enum and one CHECK constraint, and the two agree: the CHECK set equals
# the enum's string set, the SQL DEFAULT is the enum's #[default], and no
# query! string compares the column against a literal (bind the enum instead).
# An `open` vocabulary skips set equality but must still carry a CHECK.
set -euo pipefail

cd "$(dirname "$0")/.."

python3 - scripts/sql-vocabularies.txt <(git ls-files -co --exclude-standard \
    'crates/*/*/src/**/*.rs' 'crates/*/*/src/*.rs' | grep -v '^crates/tests/' | sort -u) <<'PY'
import re
import sys

registry_path, sources_path = sys.argv[1:3]
failed = False


def fail(msg):
    global failed
    failed = True
    print(f"lint-sql-vocab: {msg}", file=sys.stderr)


def column_clause(sql, table, column):
    m = re.search(r"CREATE TABLE IF NOT EXISTS\s+" + re.escape(table) + r"\s*\(", sql)
    if not m:
        return None
    depth, i = 0, m.end() - 1
    while i < len(sql):
        if sql[i] == "(":
            depth += 1
        elif sql[i] == ")":
            depth -= 1
            if depth == 0:
                break
        i += 1
    body = sql[m.end():i]
    items, depth, item = [], 0, ""
    for ch in body + ",":
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            if item.strip():
                items.append(item.strip())
            item = ""
        else:
            item += ch
    for item in items:
        words = item.split(None, 1)
        if words and words[0] == column:
            return words[1] if len(words) > 1 else ""
    return None


def check_set(sql, table, column):
    clause = column_clause(sql, table, column)
    if clause is None:
        return None, None, None
    default = re.search(r"DEFAULT\s+'([^']*)'", clause)
    check_in = re.search(r"CHECK\s*\(\s*" + re.escape(column) + r"\s+IN\s*\((.*?)\)\s*\)", clause, re.S)
    any_check = "CHECK" in clause
    values = set(re.findall(r"'([^']*)'", check_in.group(1))) if check_in else None
    return (default.group(1) if default else None), values, any_check


def enum_strings(src, name):
    m = re.search(r"pub enum " + re.escape(name) + r"\s*\{(.*?)\n\}", src, re.S)
    if not m:
        return None, None
    body = m.group(1)
    variants = re.findall(r"^\s*(?:#\[default\]\s*)?([A-Z]\w*)\s*(?:\([^)]*\))?\s*,", body, re.M)
    default = re.search(r"#\[default\]\s*([A-Z]\w*)", body)
    impl = re.search(r"impl " + re.escape(name) + r"\s*\{.*?fn as_str\(.*?\{(.*?)\n    \}", src, re.S)
    strings = {}
    if impl:
        for variant, text in re.findall(r"Self::([A-Z]\w*)(?:\([^)]*\))?\s*=>\s*\"([^\"]*)\"", impl.group(1)):
            strings[variant] = text
    return (strings, default.group(1) if default else None)


entries = []
for raw in open(registry_path, encoding="utf-8"):
    line = raw.strip()
    if not line or line.startswith("#"):
        continue
    parts = line.split("|")
    if len(parts) not in (4, 5) or (len(parts) == 5 and parts[4] != "open"):
        fail(f"malformed registry line: {line}")
        continue
    entries.append(parts)

for parts in entries:
    column_ref, schema_path, enum_path, enum_name = parts[:4]
    is_open = len(parts) == 5
    table, column = column_ref.split(".", 1)
    try:
        sql = open(schema_path, encoding="utf-8").read()
        src = open(enum_path, encoding="utf-8").read()
    except OSError as e:
        fail(f"{column_ref}: {e}")
        continue
    strings, enum_default = enum_strings(src, enum_name)
    if strings is None:
        fail(f"{column_ref}: enum `{enum_name}` not found in {enum_path}")
        continue
    sql_default, values, any_check = check_set(sql, table, column)
    if sql_default is None and values is None and any_check is None:
        fail(f"{column_ref}: column not found in {schema_path}")
        continue
    if not any_check:
        fail(f"{column_ref}: {schema_path} declares no CHECK on `{column}`")
    if sql_default is None and enum_default is not None:
        fail(f"{column_ref}: `{enum_name}` has #[default] but the column has no SQL DEFAULT")
    elif sql_default is not None and enum_default is None:
        fail(f"{column_ref}: `{enum_name}` has no #[default] variant")
    elif sql_default is not None and strings.get(enum_default) != sql_default:
        fail(f"{column_ref}: SQL DEFAULT '{sql_default}' is not `{enum_name}::{enum_default}` ({strings.get(enum_default)!r})")
    if not is_open:
        if values is None:
            fail(f"{column_ref}: {schema_path} has no CHECK ({column} IN (...))")
        else:
            enum_set = set(strings.values())
            if values != enum_set:
                fail(f"{column_ref}: CHECK set {sorted(values)} != `{enum_name}` strings {sorted(enum_set)}")

columns = {}
for parts in entries:
    table, column = parts[0].split(".", 1)
    columns.setdefault(column, set()).add(table)

macro_re = re.compile(r"sqlx::query(?:_as|_scalar|_file|_file_as|_file_scalar)?!\s*\(", re.S)
literal_res = {
    column: re.compile(r"\b" + re.escape(column) + r"\s*(?:=|<>|!=|IN\s*\()\s*'[^']*'")
    for column in columns
}

for path in (line.strip() for line in open(sources_path, encoding="utf-8") if line.strip()):
    text = open(path, encoding="utf-8").read()
    if "sqlx::query" not in text:
        continue
    for m in macro_re.finditer(text):
        depth, i = 0, m.end() - 1
        while i < len(text):
            if text[i] == "(":
                depth += 1
            elif text[i] == ")":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        call = text[m.start():i]
        for column, tables in columns.items():
            if not any(re.search(r"\b" + re.escape(t) + r"\b", call) for t in tables):
                continue
            hit = literal_res[column].search(call)
            if hit:
                lineno = text.count("\n", 0, m.start() + hit.start()) + 1
                fail(f"{path}:{lineno} -- `{hit.group(0).strip()}` is a literal; bind the enum's as_str() instead")

if failed:
    sys.exit(1)
print(f"lint-sql-vocab: OK ({len(entries)} vocabularies agree with their CHECK constraints; no status literals in query! strings)")
PY
