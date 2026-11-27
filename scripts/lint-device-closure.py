#!/usr/bin/env python3
"""Keep the server kernel and the device crates apart above the shared layer.

The device crates (`systemprompt-device`, `-machine`, `-engine`, `-proxy`,
`-hosts`, `-drive`, `-runner`, `-harness`, `-mobile`) are composed into the
binary on a user's machine or phone; the server kernel (the `kernel` rows of scripts/units.txt)
is composed into an instance. Each side has a closure: the crates it reaches
over production edges. The two closures meet only in `crates/shared/*`:

  1. No crate of the kernel closure names a device crate. The closure starts
     at every kernel crate except the `systemprompt` facade, whose features
     select the device slices, and is followed through whatever those crates
     depend on.
  2. No crate of the device closure names a crate that is neither a device
     crate nor a shared-layer crate. That refuses every server crate above
     `shared` and the facade.
  3. No device crate is a `kernel` row of scripts/units.txt.
  4. Every device crate is a member of the root workspace, so a renamed or
     removed crate cannot leave the gate with nothing to judge.

A walk stops at an edge it refuses, so one wrong edge is one line. Only
normal and build dependencies count, as in lint-layers, which holds each
device crate at its layer. The graph is `cargo metadata --offline --no-deps`
of the root workspace.

Usage: lint-device-closure.py [--metadata FILE] [--units FILE]
  --metadata FILE   read the cargo metadata JSON from FILE (fixture graphs)
  --units FILE      read the unit table from FILE instead of scripts/units.txt
"""
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from cargo_graph import load_metadata, packages_by_name, production_edges  # noqa: E402
from units import UNITS, load_units  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
DEVICE = (
    "systemprompt-device",
    "systemprompt-machine",
    "systemprompt-engine",
    "systemprompt-proxy",
    "systemprompt-hosts",
    "systemprompt-drive",
    "systemprompt-runner",
    "systemprompt-harness",
    "systemprompt-mobile",
)
SHARED = "shared"
FACADE = "facade"


def layer_of(pkg):
    parts = pkg["manifest_path"].split("/crates/")
    return parts[1].split("/")[0] if len(parts) > 1 else FACADE


def walk(seeds, deps, refused):
    """The closure of `seeds` and the edges `refused(dep)` stopped it at, both sorted."""
    seen = set(seeds)
    frontier = sorted(seeds)
    stopped = set()
    while frontier:
        name = frontier.pop()
        for dep in deps.get(name, ()):
            if refused(dep):
                stopped.add((name, dep))
            elif dep not in seen:
                seen.add(dep)
                frontier.append(dep)
    return sorted(seen), sorted(stopped)


def judge(metadata, units):
    """`(violations, shared crates both closures reach)` for a graph and a unit table."""
    pkgs = packages_by_name(metadata)
    deps = production_edges(pkgs)
    layer = {name: layer_of(pkg) for name, pkg in pkgs.items()}
    device = set(DEVICE)
    found = [f"  {name}: a device crate is a member of the root workspace" for name in DEVICE if name not in pkgs]
    found += [
        f"  {name}: a device crate is no kernel row of scripts/units.txt"
        for name in DEVICE if units.get(name) == "kernel"
    ]
    kernel = {
        name for name, unit in units.items()
        if unit == "kernel" and name in pkgs and name not in device and layer[name] != FACADE
    }
    server, into_device = walk(kernel, deps, lambda dep: dep in device)
    found += [f"  {name} -> {dep}: the server kernel closure names no device crate" for name, dep in into_device]
    reached, out_of_device = walk(
        device & set(pkgs), deps, lambda dep: dep not in device and layer[dep] != SHARED,
    )
    found += [
        f"  {name} -> {dep}: the device closure names no {layer[dep]} crate outside the device crates"
        for name, dep in out_of_device
    ]
    meet = sorted(set(server) & set(reached))
    found += [f"  {name}: reached by both closures outside the shared layer" for name in meet if layer[name] != SHARED]
    return found, [name for name in meet if layer[name] == SHARED]


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
    found, meet = judge(metadata, load_units(units_path))
    if found:
        print("\n".join(found))
        print(f"lint-device-closure: FAIL — {len(found)} device closure violation(s)")
        return 1
    print(f"lint-device-closure: OK — {len(DEVICE)} device crates, the kernel and device closures meet in {len(meet)} shared crate(s) and nowhere else")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
