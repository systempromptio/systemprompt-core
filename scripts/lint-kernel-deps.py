#!/usr/bin/env python3
"""Keep the kernel floor: no kernel crate links a capability, at any depth.

Every package whose unit in scripts/units.txt is `kernel` is judged over the
transitive closure of its normal and build dependencies (`production_edges`
of scripts/lib/cargo_graph.py). Optional dependencies count: a feature-gated
edge is still an edge of the shipped graph. A `capability:*` package in that
closure fails the gate, printed with the shortest path that reaches it.

The facade (`systemprompt`) is the composition point: its optional
capability dependencies are how a composition's features select members. It
is judged over its non-optional dependencies plus the optional ones the
kernel composition enables -- the features of the scripts/build-compositions.txt
line flagged `floor`, expanded through the facade's `[features]` table
(`dep:x` and `x/feature` enable `x`; `x?/feature` enables nothing; a
renamed dependency is enabled by its rename).

Every package of the graph has a row in the unit table: a package with none
is never judged, so it fails the gate.

A capability entry crate (`systemprompt-*-entry`) contributes through the
extension contracts, so it may not depend on `systemprompt-cli` or
`systemprompt-api`.

The graph is `cargo metadata --offline --no-deps` of the root workspace.

Usage: lint-kernel-deps.py [--metadata FILE] [--units FILE] [--compositions FILE]
  --metadata FILE       read the cargo metadata JSON from FILE (fixture graphs)
  --units FILE          the unit table (default scripts/units.txt)
  --compositions FILE   the composition table (default scripts/build-compositions.txt)
"""
from collections import deque
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from cargo_graph import load_metadata, packages_by_name, production_edges  # noqa: E402
from units import UnitsError, load_units  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
COMPOSITIONS = ROOT / "scripts" / "build-compositions.txt"
FACADE = "systemprompt"
ENTRY_FORBIDDEN = ("systemprompt-cli", "systemprompt-api")


class CompositionError(Exception):
    """scripts/build-compositions.txt names no single `floor` line."""


def is_entry_capability(name):
    return name.startswith("systemprompt-") and name.endswith("-entry")


def floor_features(text):
    """The facade features of the one composition line flagged `floor`."""
    found = []
    for raw in text.splitlines():
        fields = raw.split("#", 1)[0].split()
        if len(fields) >= 3 and "floor" in fields[2].split(","):
            found.append(fields[1].split(","))
    if len(found) != 1:
        raise CompositionError(f"build-compositions: expected exactly one line flagged floor, found {len(found)}")
    return found[0]


def enabled_optional_deps(pkg, features):
    """Optional dependency names a feature set of `pkg` turns on."""
    table = pkg.get("features", {})
    seen, deps, stack = set(), set(), list(features)
    while stack:
        feature = stack.pop()
        if feature in seen:
            continue
        seen.add(feature)
        for item in table.get(feature, ()):
            if item.startswith("dep:"):
                deps.add(item[4:])
            elif "?/" in item:
                continue
            elif "/" in item:
                deps.add(item.split("/", 1)[0])
            else:
                stack.append(item)
    return deps


def facade_edges(pkg, local, features):
    enabled = enabled_optional_deps(pkg, ["default", *features])
    return {
        d["name"] for d in pkg["dependencies"]
        if d["name"] in local and d["name"] != pkg["name"] and d["kind"] in (None, "build")
        and (not d.get("optional") or (d.get("rename") or d["name"]) in enabled)
    }


def shortest_paths(start, deps):
    parent, queue = {start: None}, deque([start])
    while queue:
        node = queue.popleft()
        for dep in sorted(deps.get(node, ())):
            if dep not in parent:
                parent[dep] = node
                queue.append(dep)
    return parent


def path_to(parent, target):
    path = [target]
    while parent[path[-1]] is not None:
        path.append(parent[path[-1]])
    return " -> ".join(reversed(path))


def violations(metadata, units, features):
    pkgs = packages_by_name(metadata)
    deps = production_edges(pkgs)
    if FACADE in pkgs:
        deps[FACADE] = facade_edges(pkgs[FACADE], set(pkgs), features)
    found = [f"  {name}: has no row in the unit table, so no gate judges it"
             for name in sorted(pkgs) if name not in units]
    for name in sorted(n for n in pkgs if units.get(n) == "kernel"):
        parent = shortest_paths(name, deps)
        for dep in sorted(parent):
            unit = units.get(dep, "")
            if unit.startswith("capability:"):
                found.append(f"  {path_to(parent, dep)}: a kernel crate links no capability ({dep} is {unit})")
    for name in sorted(pkgs):
        if not is_entry_capability(name):
            continue
        for dep in sorted(deps.get(name, set()) & set(ENTRY_FORBIDDEN)):
            found.append(f"  {name} -> {dep}: a capability entry crate contributes through systemprompt-extension")
    return found


def main(argv):
    args = dict(zip(argv[::2], argv[1::2]))
    if len(argv) % 2 or set(args) - {"--metadata", "--units", "--compositions"}:
        print(__doc__.split("Usage:")[1].strip(), file=sys.stderr)
        return 2
    try:
        units = load_units(args["--units"]) if "--units" in args else load_units()
        features = floor_features(Path(args.get("--compositions", COMPOSITIONS)).read_text())
    except (UnitsError, CompositionError) as error:
        print(f"lint-kernel-deps: {error}", file=sys.stderr)
        return 2
    if "--metadata" in args:
        metadata = json.loads(Path(args["--metadata"]).read_text())
    else:
        metadata = load_metadata(ROOT / "Cargo.toml")
    found = violations(metadata, units, features)
    if found:
        print("\n".join(found))
        print(f"lint-kernel-deps: FAIL — {len(found)} kernel floor or entry dependency violation(s)")
        return 1
    print("lint-kernel-deps: OK — no kernel crate links a capability, entry crates name no kernel entry")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
