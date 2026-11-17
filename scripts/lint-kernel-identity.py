#!/usr/bin/env python3
"""Keep owner identity typed in the kernel's mount and command registry.

The API mount (`crates/entry/api/src/services/middleware/mount/`) and the
CLI kernel (`crates/entry/cli/src/kernel/`) decide privileges by owner. An
owner is a typed `ExtensionId` compared with `CoreId::id()`, never a Rust
type name and never a string spelled at the comparison. So these trees may
not name `type_name` at all, may not hold a string literal equal to a
`CoreId` id (read from `crates/shared/extension/src/reserved/mod.rs`), and
may not compare `.as_str()` with a string literal.

Usage: lint-kernel-identity.py [ROOT]
  ROOT   repository root to scan (default: this repository; fixtures pass one)
"""
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from rust_tokens import located, rust_files  # noqa: E402

KERNEL_TREES = ("crates/entry/api/src/services/middleware/mount", "crates/entry/cli/src/kernel")
RESERVED = "crates/shared/extension/src/reserved/mod.rs"


def core_ids(root):
    tokens = located((Path(root) / RESERVED).read_text())
    ids = set()
    for i in range(len(tokens) - 4):
        if [t[:2] for t in tokens[i:i + 3]] == [("id", "Self"), ("p", ":"), ("p", ":")] and tokens[i + 4][:2] == ("p", "="):
            j = i + 5
            if j + 1 < len(tokens) and tokens[j][:2] == ("p", ">") and tokens[j + 1][0] == "str":
                ids.add(tokens[j + 1][1])
    return ids


def findings(root):
    ids = core_ids(root)
    if not ids:
        return [f"  {RESERVED}: no CoreId ids found; the gate cannot judge owner literals"]
    found = []
    for rel, path in rust_files(root, KERNEL_TREES):
        tokens = located(path.read_text())
        for i, (kind, value, line) in enumerate(tokens):
            if kind == "id" and value == "type_name":
                found.append(f"  {rel}:{line}: names type_name")
            elif kind == "str" and value in ids:
                found.append(f"  {rel}:{line}: owner literal \"{value}\"")
            elif (kind, value) == ("id", "as_str"):
                rest = [t[:2] for t in tokens[i + 1:i + 5]]
                if rest[:4] == [("p", "("), ("p", ")"), ("p", "="), ("p", "=")] and i + 5 < len(tokens) and tokens[i + 5][0] == "str":
                    found.append(f"  {rel}:{line}: compares an id with a string literal")
    return found


def main(argv):
    root = Path(argv[0]) if argv else Path(__file__).resolve().parents[1]
    found = findings(root)
    if found:
        print("\n".join(found))
        print(f"lint-kernel-identity: FAIL — {len(found)} untyped owner identity finding(s)")
        return 1
    print("lint-kernel-identity: OK — the api mount and cli kernel identify owners by type")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
