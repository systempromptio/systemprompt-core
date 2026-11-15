#!/usr/bin/env python3
"""Report Rust files in a test crate that no crate root compiles.

rustc compiles a file only when a module declaration reaches it: `mod x;`
resolves to `x.rs` or `x/mod.rs`, `#[path = "..."] mod x;` names the file
outright, and inline `mod x { ... }` blocks add a directory level for the
declarations inside them. A file on disk that nothing declares is never
built, so its tests never run and nothing says so.

This gate walks every crate under `crates/tests/` and `bin/bridge`. A crate's
roots are `build.rs`, `src/lib.rs`, `src/main.rs`, `src/bin/*.rs`,
`src/bin/*/main.rs`, `tests/*.rs`, `tests/*/main.rs`, `benches/*.rs`,
`examples/*.rs` (each auto-discovered kind unless `auto<kind> = false`) and
every `path` of a `[lib]`, `[[bin]]`, `[[test]]`, `[[bench]]` or `[[example]]`
table. From the roots it follows module declarations through a tokeniser that
skips comments and string literals, so a commented-out `mod` reaches nothing.
`#[cfg(...)]`-gated declarations count as reachable.

Reachability is global: a file one crate includes through `#[path]` is
compiled, so it is not an orphan of the crate whose directory holds it. Every
`*.rs` file under a crate directory (nested crates excluded) that no root of
any walked crate reaches is reported.

There is no allowlist. A reported file is registered in its parent module or
deleted.

Usage: lint-orphan-tests.py [ROOT...] [--paths P...]
  no arguments   every crate under crates/tests/ and bin/bridge
  ROOT...        walk the crates under these directories instead
  --paths P...   report only orphans under P (reachability is still global)
"""
import os
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import gate_scope  # noqa: E402

DEFAULT_ROOTS = ("crates/tests", "bin/bridge")
TARGET_TABLES = {"lib": "lib", "bin": "bin", "test": "test", "bench": "bench", "example": "example"}
AUTO_KEYS = {"autobins": "bin", "autotests": "test", "autobenches": "bench", "autoexamples": "example"}


def tokenize(text):
    """Identifier, punctuation and string tokens of Rust source.

    Comments (nested block comments included), char literals and lifetimes are
    dropped. A string token is `("str", value)`; the value is the raw text
    between the quotes, enough for a `#[path]` argument.
    """
    tokens = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith("//", i):
            j = text.find("\n", i)
            i = n if j < 0 else j + 1
        elif text.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if text.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif text.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif c in "rb" and raw_string_at(text, i):
            j = i + (2 if text.startswith("br", i) else 1)
            hashes = 0
            while text[j] == "#":
                hashes, j = hashes + 1, j + 1
            close = '"' + "#" * hashes
            end = text.find(close, j + 1)
            end = n if end < 0 else end
            tokens.append(("str", text[j + 1:end]))
            i = end + len(close)
        elif c == '"' or (c in "bc" and text.startswith('"', i + 1)):
            j = i + 1 if c == '"' else i + 2
            start = j
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            tokens.append(("str", text[start:j]))
            i = j + 1
        elif c == "'":
            if text.startswith("\\", i + 1):
                j = text.find("'", i + 3)
                i = n if j < 0 else j + 1
            elif i + 2 < n and text[i + 2] == "'":
                i += 3
            else:
                i += 1
        elif c.isalnum() or c == "_":
            j = i
            while j < n and (text[j].isalnum() or text[j] == "_"):
                j += 1
            word = text[i:j]
            if word == "r" and text.startswith("#", j) and j + 1 < n and (text[j + 1].isalpha() or text[j + 1] == "_"):
                k = j + 1
                while k < n and (text[k].isalnum() or text[k] == "_"):
                    k += 1
                word, j = "r#" + text[j + 1:k], k
            tokens.append(("id", word))
            i = j
        else:
            tokens.append(("p", c))
            i += 1
    return tokens


def raw_string_at(text, i):
    j = i + (2 if text.startswith("br", i) else 1)
    if text.startswith("b", i) and not text.startswith("br", i):
        return False
    if i > 0 and (text[i - 1].isalnum() or text[i - 1] == "_"):
        return False
    while j < len(text) and text[j] == "#":
        j += 1
    return j < len(text) and text[j] == '"'


def matching(tokens, i, open_, close):
    depth = 0
    for j in range(i, len(tokens)):
        if tokens[j] == ("p", open_):
            depth += 1
        elif tokens[j] == ("p", close):
            depth -= 1
            if depth == 0:
                return j
    return len(tokens) - 1


def matching_back(tokens, i, open_, close):
    depth = 0
    for j in range(i, -1, -1):
        if tokens[j] == ("p", close):
            depth += 1
        elif tokens[j] == ("p", open_):
            depth -= 1
            if depth == 0:
                return j
    return 0


def path_attribute(tokens, mod_at):
    """The `path = "..."` argument of an outer attribute on the `mod` at `mod_at`."""
    j = mod_at - 1
    if j >= 0 and tokens[j] == ("p", ")"):
        j = matching_back(tokens, j, "(", ")") - 1
    if j >= 0 and tokens[j] == ("id", "pub"):
        j -= 1
    found = None
    while j >= 1 and tokens[j] == ("p", "]"):
        start = matching_back(tokens, j, "[", "]")
        if start < 1 or tokens[start - 1] != ("p", "#"):
            break
        for k in range(start + 1, j - 2):
            if tokens[k] == ("id", "path") and tokens[k + 1] == ("p", "=") and tokens[k + 2][0] == "str":
                found = tokens[k + 2][1]
        j = start - 2
    return found


def declarations(tokens):
    """`(path_attr, inline_names, name)` for each out-of-line `mod name;`.

    `inline_names` lists the enclosing inline `mod x { ... }` blocks, outermost
    first.
    """
    out = []
    stack = []
    depth = 0
    i = 0
    while i < len(tokens):
        tok = tokens[i]
        if tok == ("p", "{"):
            depth += 1
        elif tok == ("p", "}"):
            depth -= 1
            while stack and stack[-1][1] > depth:
                stack.pop()
        elif (tok == ("id", "mod") and i + 2 < len(tokens) and tokens[i + 1][0] == "id"
              and (i == 0 or tokens[i - 1] != ("p", "$"))):
            name = tokens[i + 1][1]
            name = name[2:] if name.startswith("r#") else name
            after = tokens[i + 2]
            if after == ("p", ";"):
                out.append((path_attribute(tokens, i), [s[0] for s in stack], name))
                i += 3
                continue
            if after == ("p", "{"):
                depth += 1
                stack.append((name, depth, path_attribute(tokens, i)))
                i += 3
                continue
        i += 1
    return out


def resolve(file, mod_rs, decl):
    """The file a declaration in `file` names, or None when none exists."""
    path_attr, inline, name = decl
    parent = file.parent
    if path_attr is not None:
        base = parent if mod_rs or not inline else parent / file.stem
        target = base.joinpath(*inline) / path_attr
        return target if target.is_file() else None
    base = parent if mod_rs else parent / file.stem
    base = base.joinpath(*inline)
    for candidate in (base / f"{name}.rs", base / name / "mod.rs"):
        if candidate.is_file():
            return candidate
    return None


def reachable(roots):
    """Every file reached from `roots`; a root and a `#[path]` file are mod-rs-like."""
    seen = {}
    work = [(Path(os.path.normpath(r)), True) for r in roots]
    while work:
        file, mod_rs = work.pop()
        if file in seen:
            continue
        seen[file] = mod_rs
        try:
            text = file.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for decl in declarations(tokenize(text)):
            target = resolve(file, mod_rs, decl)
            if target is None:
                continue
            target = Path(os.path.normpath(target))
            target_mod_rs = decl[0] is not None or target.name == "mod.rs"
            work.append((target, target_mod_rs))
    return set(seen)


def manifest_targets(manifest):
    """`(explicit paths, auto kinds switched off)` read from a Cargo.toml."""
    paths, off = [], set()
    table = None
    for raw in manifest.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line.startswith("["):
            name = line.strip("[]").strip()
            table = TARGET_TABLES.get(name, "package" if name == "package" else None)
            continue
        if "=" not in line:
            continue
        key, value = (part.strip() for part in line.split("=", 1))
        value = value.strip('"')
        if table == "package" and key == "build" and value not in ("true", "false"):
            paths.append(value)
        elif table == "package" and key in AUTO_KEYS and value == "false":
            off.add(AUTO_KEYS[key])
        elif table in TARGET_TABLES.values() and key == "path":
            paths.append(value)
    return paths, off


def crate_roots(crate):
    paths, off = manifest_targets(crate / "Cargo.toml")
    roots = [crate / p for p in paths]
    auto = [crate / "build.rs", crate / "src" / "lib.rs", crate / "src" / "main.rs"]
    globs = {"bin": ["src/bin/*.rs", "src/bin/*/main.rs"], "test": ["tests/*.rs", "tests/*/main.rs"],
             "bench": ["benches/*.rs", "benches/*/main.rs"], "example": ["examples/*.rs", "examples/*/main.rs"]}
    for kind, patterns in globs.items():
        if kind not in off:
            for pattern in patterns:
                auto.extend(crate.glob(pattern))
    return [r for r in roots + auto if r.is_file()]


def crates(roots):
    manifests = []
    for root in roots:
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if not d.startswith(("target", ".")) and d != "node_modules"]
            if "Cargo.toml" in filenames:
                manifests.append(Path(dirpath))
    return sorted(manifests)


def source_files(root):
    """`*.rs` files under `root`: git-listed (tracked and untracked-not-ignored)
    inside the repository, every file on disk outside it."""
    absolute = Path(root).resolve()
    if absolute == gate_scope.ROOT or gate_scope.ROOT in absolute.parents:
        rel = absolute.relative_to(gate_scope.ROOT).as_posix()
        return [gate_scope.ROOT / f for f in gate_scope.files([f"{rel}/**.rs" if rel else "*.rs"])]
    out = []
    for dirpath, dirnames, filenames in os.walk(absolute):
        dirnames[:] = [d for d in dirnames if not d.startswith(("target", ".")) and d != "node_modules"]
        out.extend(Path(dirpath) / f for f in filenames if f.endswith(".rs"))
    return out


def owner(file, all_crates):
    """The innermost crate directory holding `file`."""
    holders = [c for c in all_crates if c in file.parents]
    return max(holders, key=lambda c: len(c.parts)) if holders else None


def orphans(roots):
    found = [c.resolve() for c in crates(roots)]
    reached = {f.resolve() for f in reachable([r for crate in found for r in crate_roots(crate)])}
    candidates = {f.resolve() for root in roots for f in source_files(root)}
    return sorted(f for f in candidates - reached if owner(f, found) is not None)


def main(argv):
    try:
        args, scope = gate_scope.parse(argv)
    except gate_scope.ScopeError as error:
        print(f"lint-orphan-tests: {error}", file=sys.stderr)
        return 2
    if args and scope is not None:
        print("lint-orphan-tests: pass ROOTs or --paths, pass one, not both", file=sys.stderr)
        return 2
    os.chdir(gate_scope.ROOT)
    roots = args or [r for r in DEFAULT_ROOTS if Path(r).is_dir()]
    hits = [gate_scope.relative(f) for f in orphans(roots)]
    hits = [f for f in hits if gate_scope.in_scope(f, scope)]
    if hits:
        for f in hits:
            print(f"{f}: no crate root reaches this file")
        print(f"lint-orphan-tests: {len(hits)} file(s) are never compiled; declare each in its "
              "parent module or delete it", file=sys.stderr)
        return 1
    print("lint-orphan-tests: OK (every test source file is reachable from a crate root)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
