#!/usr/bin/env python3
"""Keep link-time registries out of the root workspace.

A composition names its members; nothing is collected at link time. So no
production source in the root workspace (`crates/` outside the separate
`crates/tests` workspace, and the `systemprompt/` facade) may name the
`inventory` crate: no `inventory::` path (`inventory::submit!`,
`inventory::collect!`, `inventory::iter`), no `use inventory` and no
`extern crate inventory`. `bin/bridge` is its own workspace and keeps its
registries.

Usage: lint-no-inventory.py [ROOT]
  ROOT   repository root to scan (default: this repository; fixtures pass one)
"""
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from rust_tokens import located, rust_files  # noqa: E402

SCANNED = ("crates", "systemprompt")
EXCLUDED = ("crates/tests",)


def findings(root):
    found = []
    for rel, path in rust_files(root, SCANNED, EXCLUDED):
        tokens = located(path.read_text())
        for i, (kind, value, line) in enumerate(tokens):
            if (kind, value) != ("id", "inventory"):
                continue
            path_use = tokens[i + 1:i + 3] and [t[:2] for t in tokens[i + 1:i + 3]] == [("p", ":"), ("p", ":")]
            prev = tokens[i - 1][:2] if i else None
            if path_use or prev in (("id", "use"), ("id", "crate")):
                found.append(f"  {rel}:{line}: names the inventory crate")
    return found


def main(argv):
    root = Path(argv[0]) if argv else Path(__file__).resolve().parents[1]
    found = findings(root)
    if found:
        print("\n".join(found))
        print(f"lint-no-inventory: FAIL — {len(found)} inventory use(s) in the root workspace")
        return 1
    print("lint-no-inventory: OK — the root workspace names no inventory registry")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
