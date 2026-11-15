#!/usr/bin/env python3
"""Keep `Contributions` a write-only view of one member's own sink.

A member's `contribute` receives a `Contributions` bound to its own sink. If
any method handed back a mutable reference to stored entries, a member could
rewrite what it, or another member, declared after the fact. So every `pub`
method in an `impl ... Contributions` block in
`crates/shared/extension/src/contribute/mod.rs` returns one of `Self`,
`&mut Self` (chaining), `&mut S::Builder` (the member's own slot builder) or
`&KernelView` (the read-only kernel view), or nothing; and the struct has no
`pub` field.

Usage: lint-contributions-surface.py [ROOT]
  ROOT   repository root to scan (default: this repository; fixtures pass one)
"""
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from rust_tokens import located, matching  # noqa: E402

SOURCE = "crates/shared/extension/src/contribute/mod.rs"
ALLOWED_RETURNS = re.compile(r"Self|&mutSelf|&mutS::Builder|&KernelView(<\w*>)?")


def blocks(tokens, keyword):
    for i, token in enumerate(tokens):
        if token[:2] != ("id", keyword):
            continue
        j = i + 1
        while j < len(tokens) and tokens[j][:2] not in (("p", "{"), ("p", ";")):
            j += 1
        header = [t[1] for t in tokens[i + 1:j]]
        if j < len(tokens) and tokens[j][:2] == ("p", "{") and "Contributions" in header and "for" not in header:
            if keyword == "struct" and header[0] != "Contributions":
                continue
            yield j, matching([t[:2] for t in tokens], j, "{", "}")


def findings(root):
    path = Path(root) / SOURCE
    if not path.exists():
        return [f"  {SOURCE}: missing"]
    tokens = located(path.read_text())
    found = []
    impls = list(blocks(tokens, "impl"))
    if not impls:
        found.append(f"  {SOURCE}: no impl Contributions block found")
    for start, end in impls:
        depth = 0
        for i in range(start, end):
            kind, value, line = tokens[i]
            if (kind, value) == ("p", "{"):
                depth += 1
            elif (kind, value) == ("p", "}"):
                depth -= 1
            if depth != 1 or (kind, value) != ("id", "pub"):
                continue
            j = i + 1
            while j < end and tokens[j][:2] != ("id", "fn"):
                if tokens[j][:2] in (("p", ";"), ("p", "{")):
                    break
                j += 1
            if j >= end or tokens[j][:2] != ("id", "fn"):
                continue
            name = tokens[j + 1][1]
            k = j + 2
            ret = None
            while k < end and tokens[k][:2] not in (("p", "{"), ("id", "where"), ("p", ";")):
                if tokens[k][:2] == ("p", "-") and tokens[k + 1][:2] == ("p", ">"):
                    ret, k = [], k + 2
                    continue
                if ret is not None:
                    ret.append(tokens[k][1])
                k += 1
            if ret is not None and not ALLOWED_RETURNS.fullmatch("".join(ret)):
                found.append(f"  {SOURCE}:{line}: Contributions::{name} returns {''.join(ret)}")
    for start, end in blocks(tokens, "struct"):
        for i in range(start + 1, end):
            if tokens[i][:2] == ("id", "pub"):
                found.append(f"  {SOURCE}:{tokens[i][2]}: Contributions has a pub field")
    return found


def main(argv):
    root = Path(argv[0]) if argv else Path(__file__).resolve().parents[1]
    found = findings(root)
    if found:
        print("\n".join(found))
        print(f"lint-contributions-surface: FAIL — {len(found)} finding(s)")
        return 1
    print("lint-contributions-surface: OK — Contributions hands out no stored entry")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
