#!/usr/bin/env python3
"""Keep every capability free of every other capability.

A capability (a `capability:<name>` row of scripts/units.txt: its domain
crate and its entry crate) reaches another capability only through the
shared-layer contracts the kernel wires at a composition root. So no package
of one capability may depend on a package of a different capability; a
dependency between two packages of the same capability (the entry crate on its
domain crate) is the pair itself.

Only normal and build dependencies count, as in lint-layers, and an optional
dependency counts: a feature that links another capability is still a
capability -> capability edge. The graph is `cargo metadata --offline
--no-deps` of the root workspace.

Usage: lint-capability-deps.py [--metadata FILE] [--units FILE]
  --metadata FILE   read the cargo metadata JSON from FILE (fixture graphs)
  --units FILE      read the unit table from FILE instead of scripts/units.txt
"""
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from cargo_graph import capability_edges, load_metadata  # noqa: E402
from units import UNITS, load_units  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]


def violations(metadata, units):
    return [
        f"  {name} -> {dep}: {units[name]} depends on {units[dep]}; reach it through a shared-layer contract"
        for name, dep in capability_edges(metadata, units)
    ]


def main(argv):
    metadata_path = None
    units_path = UNITS
    args = list(argv)
    while args:
        flag = args.pop(0)
        if flag in ("--metadata", "--units") and args:
            value = Path(args.pop(0))
            if flag == "--metadata":
                metadata_path = value
            else:
                units_path = value
        else:
            print(__doc__.split("Usage:")[1].strip(), file=sys.stderr)
            return 2
    metadata = json.loads(metadata_path.read_text()) if metadata_path else load_metadata(ROOT / "Cargo.toml")
    found = violations(metadata, load_units(units_path))
    if found:
        print("\n".join(found))
        print(f"lint-capability-deps: FAIL — {len(found)} capability -> capability dependency(ies)")
        return 1
    print("lint-capability-deps: OK — no capability depends on another capability")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
