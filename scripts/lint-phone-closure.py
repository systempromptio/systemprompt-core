#!/usr/bin/env python3
"""Hold the phone composition to the device and client slices a phone uses.

`systemprompt-mobile` is the binding layer the iOS and Android apps link. A
phone signs in, syncs and renders what the instance records; it runs no
engine, proxy, host integration, machine policy, drive or runner. The gate
walks the crates `systemprompt-mobile` reaches over production edges, with
every feature of `systemprompt-mobile` itself enabled, and refuses:

  1. `systemprompt-mobile` missing from the root workspace.
  2. An edge to a device crate outside the phone's slices: only
     `systemprompt-device` and `systemprompt-harness` (its client state
     machines) are reached.
  3. An edge to a crate that is neither one of those nor a shared-layer
     crate. That refuses every server crate above `shared` and the facade.

Optional dependencies count only when a feature the walk has enabled turns
them on, so a slice `systemprompt-harness` gates behind a feature the phone
does not name is not in the closure. Features are resolved from the manifest
tables (`dep:x`, `x/f`, `x?/f`, default features), which is what the
no-deps metadata carries. A walk stops at an edge it refuses, so one wrong
edge is one line. Only normal and build dependencies count.

Usage: lint-phone-closure.py [--metadata FILE]
  --metadata FILE   read the cargo metadata JSON from FILE (fixture graphs)
"""
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from cargo_graph import PRODUCTION_KINDS, load_metadata, packages_by_name  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
PHONE = "systemprompt-mobile"
PHONE_SLICES = ("systemprompt-device", "systemprompt-harness")
SHARED = "shared"
FACADE = "facade"


def layer_of(pkg):
    parts = pkg["manifest_path"].split("/crates/")
    return parts[1].split("/")[0] if len(parts) > 1 else FACADE


def expand(pkg, requested):
    """`(features, optional dependencies, {dependency: features})` that `requested` enables in `pkg`."""
    table = pkg.get("features", {})
    features, optional, forwarded = set(), set(), {}
    pending = list(requested)
    while pending:
        item = pending.pop()
        if item.startswith("dep:"):
            optional.add(item[4:])
        elif "/" in item:
            dep, feature = item.split("/", 1)
            if not dep.endswith("?"):
                optional.add(dep)
            forwarded.setdefault(dep.rstrip("?"), set()).add(feature)
        elif item in table:
            if item not in features:
                features.add(item)
                pending.extend(table[item])
        elif item != "default":
            optional.add(item)
    return features, optional, forwarded


def judge(metadata):
    """The sorted refused edges of the phone closure, or the missing-crate line."""
    pkgs = packages_by_name(metadata)
    if PHONE not in pkgs:
        return [f"  {PHONE}: the phone binding crate is a member of the root workspace"]
    layer = {name: layer_of(pkg) for name, pkg in pkgs.items()}
    allowed = {PHONE, *PHONE_SLICES}
    requested = {PHONE: set(pkgs[PHONE].get("features", {})) | {"default"}}
    frontier = [PHONE]
    refused = set()
    while frontier:
        name = frontier.pop()
        _, optional, forwarded = expand(pkgs[name], requested[name])
        for dep in pkgs[name]["dependencies"]:
            target = dep["name"]
            if target not in pkgs or target == name or dep["kind"] not in PRODUCTION_KINDS:
                continue
            key = dep.get("rename") or target
            if dep.get("optional") and key not in optional:
                continue
            if target not in allowed and layer[target] != SHARED:
                refused.add((name, target))
                continue
            wanted = set(dep.get("features", ())) | forwarded.get(key, set())
            if dep.get("uses_default_features", True):
                wanted.add("default")
            if not wanted <= requested.get(target, set()) or target not in requested:
                requested[target] = requested.get(target, set()) | wanted
                frontier.append(target)
    return [
        f"  {name} -> {dep}: the phone closure names no {layer[dep]} crate outside the device and client slices"
        for name, dep in sorted(refused)
    ]


def main(argv):
    metadata_path = None
    args = list(argv)
    while args:
        flag = args.pop(0)
        if flag == "--metadata" and args:
            metadata_path = Path(args.pop(0))
        else:
            print(__doc__.split("Usage:")[1].strip(), file=sys.stderr)
            return 2
    metadata = json.loads(metadata_path.read_text()) if metadata_path else load_metadata(ROOT / "Cargo.toml")
    found = judge(metadata)
    if found:
        print("\n".join(found))
        print(f"lint-phone-closure: FAIL — {len(found)} phone closure violation(s)")
        return 1
    print(f"lint-phone-closure: OK — {PHONE} reaches only {', '.join(PHONE_SLICES)} and the shared layer")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
