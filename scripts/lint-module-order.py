#!/usr/bin/env python3
"""Keep the modules of the engine and harness crates in their dependency order.

`systemprompt-engine` and `systemprompt-harness` each hold what were separate
crates, one top-level module per crate, and the order those crates depended in
is now the order of the modules:

  systemprompt-engine   config, store, llm, tools, mcp, workspace, lsp, prompt, run
  systemprompt-harness  projection, extension, governance, members, sync

A module names only the modules before it. The four members of
`systemprompt-harness` (`members/*`) share one place in its order.

  1. No file of a module names a later module of its crate. A name is the
     first segment of a path rooted at the crate: `crate::<module>`, a
     `crate::{..}` group, `$crate::<module>` in a macro, the crate's own name
     as an extern path, or a `super::` chain that climbs to the crate root.
  2. Every top-level module of the crate has a place in the order, so a new
     module is placed before it is used.
  3. Every module of the order exists, so the gate cannot pass because its
     subjects were renamed away.

`lib.rs` declares the modules and is not judged. Comments, doc links and
string literals are not paths.

Usage: lint-module-order.py [ROOT]
  ROOT   repository root to scan (default: this repository; fixtures pass one)
"""
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from rust_tokens import located  # noqa: E402

ORDERS = {
    "crates/domain/engine/src": (
        "systemprompt_engine",
        ("config", "store", "llm", "tools", "mcp", "workspace", "lsp", "prompt", "run"),
    ),
    "crates/app/harness/src": (
        "systemprompt_harness",
        ("projection", "extension", "governance", "members", "sync"),
    ),
}


def is_sep(tokens, i):
    return i + 1 < len(tokens) and tokens[i][:2] == ("p", ":") and tokens[i + 1][:2] == ("p", ":")


def first_segments(tokens, i):
    """`(name, line)` of each first segment of the path or `{..}` group at `i`."""
    if i >= len(tokens):
        return []
    kind, value, line = tokens[i]
    if kind == "id":
        return [(value, line)]
    if (kind, value) != ("p", "{"):
        return []
    found, depth, lead = [], 0, False
    for kind, value, line in tokens[i:]:
        if (kind, value) == ("p", "{"):
            depth += 1
            lead = depth == 1
        elif (kind, value) == ("p", "}"):
            depth -= 1
            if depth == 0:
                break
        elif (kind, value) == ("p", ",") and depth == 1:
            lead = True
        else:
            if lead and kind == "id":
                found.append((value, line))
            lead = False
    return found


def rooted_names(tokens, depth, crate):
    """`(name, line)` of every first segment of a path rooted at the crate.

    `depth` is the number of modules between the crate root and the file's own
    module, itself included; an inline `mod name { .. }` adds one inside its
    braces, which is what a `super::` chain is counted against.
    """
    found, inline, braces = [], [], 0
    i, n = 0, len(tokens)
    while i < n:
        kind, value, _ = tokens[i]
        if (kind, value) == ("p", "{"):
            braces += 1
        elif (kind, value) == ("p", "}"):
            if inline and inline[-1] == braces:
                inline.pop()
            braces -= 1
        elif kind == "id" and value == "mod":
            if i + 2 < n and tokens[i + 1][0] == "id" and tokens[i + 2][:2] == ("p", "{"):
                inline.append(braces + 1)
        elif kind == "id" and value in ("crate", crate) and is_sep(tokens, i + 1):
            found += first_segments(tokens, i + 3)
        elif kind == "id" and value == "super" and not (i >= 2 and is_sep(tokens, i - 2)):
            climbed, j = 0, i
            while j < n and tokens[j][:2] == ("id", "super") and is_sep(tokens, j + 1):
                climbed, j = climbed + 1, j + 3
            if climbed and climbed == depth + len(inline):
                found += first_segments(tokens, j)
            i = max(j, i + 1)
            continue
        i += 1
    return found


def module_path(rel):
    """The modules from the crate root down to the file's own, for a path under `src`."""
    parts = list(rel.parts)
    parts[-1] = parts[-1].removesuffix(".rs")
    if parts[-1] == "mod":
        parts.pop()
    return parts


def judge(root, src, crate, order):
    """`(findings, files judged)` for one crate."""
    base = root / src
    place = {name: index for index, name in enumerate(order)}
    present = {p.name.removesuffix(".rs") for p in base.iterdir()} if base.is_dir() else set()
    found = [f"  {src}: no module `{name}`, which the order of {crate} names" for name in order if name not in present]
    judged = 0
    for path in sorted(base.rglob("*.rs")) if base.is_dir() else ():
        rel = path.relative_to(base)
        modules = module_path(rel)
        if modules in (["lib"], ["main"]):
            continue
        own = modules[0]
        if own not in place:
            found.append(f"  {src}/{rel.as_posix()}:1: module `{own}` has no place in the order of {crate}")
            continue
        judged += 1
        for name, line in rooted_names(located(path.read_text()), len(modules), crate):
            if place.get(name, -1) > place[own]:
                found.append(
                    f"  {src}/{rel.as_posix()}:{line}: `{own}` names `{name}`, which comes after it in {crate}"
                )
    return list(dict.fromkeys(found)), judged


def main(argv):
    root = Path(argv[0]) if argv else Path(__file__).resolve().parents[1]
    found, judged = [], 0
    for src, (crate, order) in ORDERS.items():
        lines, count = judge(root, src, crate, order)
        found += lines
        judged += count
    if found:
        print("\n".join(found))
        print(f"lint-module-order: FAIL — {len(found)} module order violation(s)")
        return 1
    print(f"lint-module-order: OK — {judged} files of {len(ORDERS)} crates name no later module")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
