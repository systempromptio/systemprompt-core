#!/usr/bin/env python3
"""Keep the HTTP and CLI stacks out of the domain crates.

A domain crate (`crates/domain/*`) holds a capability's behaviour; its HTTP
routes and its commands live in the capability's entry crate. So a domain
crate depends on `axum`, `tower`, `tower-http` or `clap` only as an
optional dependency that no feature enables except one named `router`,
`http` or `cli`.

Only normal and build dependencies count. The graph is `cargo metadata
--offline --no-deps` of the root workspace.

Usage: lint-domain-protocol-deps.py [--metadata FILE]
  --metadata FILE   read the cargo metadata JSON from FILE (fixture graphs)
"""
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from cargo_graph import PRODUCTION_KINDS, load_metadata  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL_CRATES = ("axum", "clap", "tower", "tower-http")
PROTOCOL_FEATURES = ("router", "http", "cli")


def is_domain(pkg):
    return "/crates/domain/" in pkg["manifest_path"]


def enabling_features(pkg, dep_name):
    markers = (f"dep:{dep_name}", dep_name, f"{dep_name}/", f"{dep_name}?/")
    return sorted(
        feature for feature, enables in pkg.get("features", {}).items()
        if any(item == markers[0] or item == markers[1] or item.startswith(markers[2:]) for item in enables)
    )


def violations(metadata):
    found = []
    for pkg in sorted(metadata["packages"], key=lambda p: p["name"]):
        if not is_domain(pkg):
            continue
        for dep in sorted(pkg["dependencies"], key=lambda d: d["name"]):
            if dep["name"] not in PROTOCOL_CRATES or dep["kind"] not in PRODUCTION_KINDS:
                continue
            if not dep.get("optional"):
                found.append(f"  {pkg['name']} -> {dep['name']}: a domain crate links no HTTP or CLI stack unconditionally")
                continue
            stray = [f for f in enabling_features(pkg, dep["name"]) if f not in PROTOCOL_FEATURES]
            if stray:
                found.append(f"  {pkg['name']} -> {dep['name']}: enabled by feature(s) {', '.join(stray)}; only router, http or cli may")
    return found


def main(argv):
    if argv[:1] == ["--metadata"] and len(argv) == 2:
        metadata = json.loads(Path(argv[1]).read_text())
    elif not argv:
        metadata = load_metadata(ROOT / "Cargo.toml")
    else:
        print(__doc__.split("Usage:")[1].strip(), file=sys.stderr)
        return 2
    found = violations(metadata)
    if found:
        print("\n".join(found))
        print(f"lint-domain-protocol-deps: FAIL — {len(found)} domain protocol dependency(ies)")
        return 1
    print("lint-domain-protocol-deps: OK — domain crates link axum, tower and clap only behind router/http/cli")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
