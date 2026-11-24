#!/usr/bin/env python3
"""Report `.as_str()` / `.to_string()` inside a sqlx query macro's bind arguments.

Every typed identifier in `systemprompt_identifiers` derives a transparent
`sqlx::Type`, so it binds as itself: `user_id as _` where `user_id: &UserId`.
Converting it to text at the bind site throws the type away exactly where a
wrong id would be caught, and the same holds for a closed vocabulary that
already derives `sqlx::Type`. The gate reads every `query!`, `query_as!`,
`query_scalar!` and `query_file*!` invocation (`sqlx::` prefix optional,
`_unchecked` forms included), skips the SQL string (and the row type or file
path), and reports any remaining argument that calls `.as_str()` or
`.to_string()`, or passes `Type::as_str` / `Type::to_string` as a function
(`.map(UserId::as_str)`). Each conversion carries a category -- `identifier`
when the receiver is id-shaped, `enum` for a variant path such as
`Status::Active`, `other` otherwise. An `identifier` conversion is a finding;
the other categories are counted on the OK line, since a vocabulary without
`sqlx::Type` or a computed text value legitimately binds as text.

Usage: lint-typed-binds.py [ROOT] [--paths P...] -- with ROOT, every `.rs` under
ROOT/crates is scanned (a test fixture); without it, the repository's tracked
files under crates/, narrowed by `--paths`. crates/tests/ is never scanned.
"""
import os
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import gate_scope  # noqa: E402

GATE = "lint-typed-binds"
SKIP_PREFIXES = ("crates/tests/",)
MACRO_RE = re.compile(
    r"(?<![\w:])(?:sqlx::)?(query|query_as|query_scalar|query_file|query_file_as|query_file_scalar)"
    r"(?:_unchecked)?!\s*\("
)
TYPED_FIRST = {"query_as", "query_file_as"}
CONVERSION_RE = re.compile(r"\.\s*(as_str|to_string)\s*\(\s*\)|(?<=\w)::(as_str|to_string)\b")
ID_SEGMENT_RE = re.compile(r"(?:^|_)(?:id|ids)$", re.I)
VARIANT_RE = re.compile(r"(?:\w+::)*[A-Z]\w*::[A-Z]\w*")
STRING_LITERAL_RE = re.compile(r'^(?:r#*)?"', re.S)


def skip_string(text, i):
    """Index just past the string literal starting at text[i] (a quote or an r)."""
    if text[i] == "r":
        j = i + 1
        hashes = 0
        while text[j] == "#":
            hashes += 1
            j += 1
        close = '"' + "#" * hashes
        end = text.find(close, j + 1)
        return len(text) if end < 0 else end + len(close)
    j = i + 1
    while j < len(text):
        if text[j] == "\\":
            j += 2
            continue
        if text[j] == '"':
            return j + 1
        j += 1
    return len(text)


def split_args(text, start):
    """Top-level comma-separated arguments of the call whose '(' is at start."""
    depth = 0
    args, current = [], []
    i = start
    n = len(text)
    while i < n:
        c = text[i]
        if c == "/" and text.startswith("//", i):
            end = text.find("\n", i)
            i = n if end < 0 else end
            continue
        if c == "/" and text.startswith("/*", i):
            end = text.find("*/", i + 2)
            i = n if end < 0 else end + 2
            continue
        if c == '"' or (c == "r" and re.match(r'r#*"', text[i:i + 8]) and not re.match(r"\w", text[i - 1])):
            j = skip_string(text, i)
            if depth >= 1:
                current.append(text[i:j])
            i = j
            continue
        if c == "'" and re.match(r"'(?:\\.|[^\\'])'", text[i:i + 6]):
            m = re.match(r"'(?:\\.[^']*|[^\\'])'", text[i:])
            current.append(m.group(0))
            i += len(m.group(0))
            continue
        if c in "([{":
            depth += 1
            if depth == 1:
                i += 1
                continue
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                tail = "".join(current).strip()
                if tail:
                    args.append(tail)
                return args, i
        elif c == "," and depth == 1:
            args.append("".join(current).strip())
            current = []
            i += 1
            continue
        current.append(c)
        i += 1
    return args, n


def category(arg, match):
    if match.group(2):
        owner = re.split(r"[^\w]", arg[: match.start()])[-1]
        return "identifier" if re.search(r"Id$", owner) else "other"
    receiver = arg[: match.start()].strip()
    if VARIANT_RE.fullmatch(receiver):
        return "enum"
    receiver = re.sub(r"[()&*]", "", receiver).strip()
    last = re.split(r"[.:\s]+", receiver)[-1] if receiver else ""
    last = re.sub(r"\(.*$", "", last)
    return "identifier" if ID_SEGMENT_RE.search(last) else "other"


def findings_in(rel, text):
    found = []
    for macro in MACRO_RE.finditer(text):
        open_at = macro.end() - 1
        args, _ = split_args(text, open_at)
        if macro.group(1) in TYPED_FIRST and args:
            args = args[1:]
        if args and STRING_LITERAL_RE.match(args[0]):
            args = args[1:]
        line = text[: macro.start()].count("\n") + 1
        for arg in args:
            for conv in CONVERSION_RE.finditer(arg):
                found.append((rel, line, category(arg, conv), " ".join(arg.split())))
    return found


def candidate_files(root, scope):
    if root is not None:
        base = Path(root)
        for dirpath, _, names in os.walk(base / "crates"):
            for name in sorted(names):
                if name.endswith(".rs"):
                    full = Path(dirpath) / name
                    yield full, full.relative_to(base).as_posix()
        return
    for rel in gate_scope.files(["crates"], scope):
        if rel.endswith(".rs"):
            yield gate_scope.ROOT / rel, rel


def main():
    try:
        args, scope = gate_scope.parse(sys.argv[1:])
    except gate_scope.ScopeError as error:
        print(f"{GATE}: {error}", file=sys.stderr)
        return 2
    if len(args) > 1:
        print(f"{GATE}: at most one ROOT", file=sys.stderr)
        return 2
    root = args[0] if args else None
    findings = []
    for full, rel in candidate_files(root, scope):
        if rel.startswith(SKIP_PREFIXES):
            continue
        try:
            text = full.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        if "query" not in text:
            continue
        findings.extend(findings_in(rel, text))
    other = [f for f in findings if f[2] != "identifier"]
    findings = [f for f in findings if f[2] == "identifier"]
    if findings:
        print(f"{GATE}: {len(findings)} bind argument(s) converted to text; bind the value as itself (`x as _`):")
        for rel, line, cat, arg in sorted(findings):
            print(f"  {rel}:{line}: [{cat}] {arg}")
        print(f"{GATE}: FAILED")
        return 1
    print(f"{GATE}: OK ({len(other)} non-identifier text bind(s) not judged)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
