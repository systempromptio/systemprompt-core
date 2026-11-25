#!/usr/bin/env python3
"""Keep capability `cfg` on module declarations and composition roots.

A capability feature switches a whole module in or out; it never edits the
inside of a function, an impl, a struct or a constant. So a
`#[cfg(...)]`/`#[cfg_attr(...)]` naming a capability feature must annotate a
`mod` declaration or the `use`/`pub use` that re-exports a gated module.
The facade crate (`systemprompt/src`) is the composition root and is exempt.

A capability feature is a capability unit name of scripts/units.txt, or a
feature of the facade (`systemprompt/Cargo.toml`) that enables a package of
a capability unit, except the protocol features (`api`, `cli`) and the
composition feature (`reference`), which link every capability's surface.

Usage: lint-capability-cfg.py [--root DIR]
  --root DIR   scan DIR (a repository layout: units, facade, crates/) instead
               of this repository
"""
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from units import parse_units  # noqa: E402

FEATURE = re.compile(r'feature\s*=\s*"([^"]+)"')
CFG = re.compile(r"^\s*#\[cfg(_attr)?\(")
ATTRIBUTE = re.compile(r"^\s*#\[")
ALLOWED_ITEM = re.compile(r"^\s*(pub(\([a-z]+\))?\s+)?(mod|use)\s")
FEATURE_ROW = re.compile(r"^([A-Za-z0-9_-]+)\s*=\s*\[(.*?)\]", re.S | re.M)
QUOTED = re.compile(r'"([^"]*)"')
SURFACE_FEATURES = ("api", "cli", "runtime", "reference")


def facade_features(text):
    text = "\n" + text
    block = text.split("\n[features]\n", 1)[1] if "\n[features]\n" in text else ""
    block = re.split(r"^\[", block, maxsplit=1, flags=re.M)[0]
    block = "\n".join(line.split("#", 1)[0] for line in block.splitlines())
    return {name: QUOTED.findall(body) for name, body in FEATURE_ROW.findall(block)}


def capability_features(root):
    units = parse_units((root / "scripts" / "units.txt").read_text())
    capability_packages = {name for name, unit in units.items() if unit.startswith("capability:")}
    names = {unit.split(":", 1)[1] for unit in units.values() if unit.startswith("capability:")}
    for feature, enables in facade_features((root / "systemprompt" / "Cargo.toml").read_text()).items():
        if feature in SURFACE_FEATURES:
            continue
        for item in enables:
            package = item.removeprefix("dep:").split("/", 1)[0].rstrip("?")
            if package in capability_packages:
                names.add(feature)
                break
    return names


def sources(root):
    for path in sorted((root / "crates").rglob("*.rs")):
        rel = path.relative_to(root).as_posix()
        if rel.startswith("crates/tests/") or "/target/" in rel or rel.endswith("/build.rs"):
            continue
        yield rel, path


def violations(root):
    capabilities = capability_features(root)
    found = []
    for rel, path in sources(root):
        lines = path.read_text().splitlines()
        for number, line in enumerate(lines, 1):
            if not CFG.match(line):
                continue
            named = sorted(set(FEATURE.findall(line)) & capabilities)
            if not named:
                continue
            item = next((later for later in lines[number:] if later.strip() and not ATTRIBUTE.match(later)), "")
            if not ALLOWED_ITEM.match(item):
                found.append(f"  {rel}:{number}: capability cfg ({', '.join(named)}) on `{item.strip()[:60]}`; gate a module instead")
    return found


def main(argv):
    root = Path(__file__).resolve().parents[1]
    if argv[:1] == ["--root"] and len(argv) == 2:
        root = Path(argv[1]).resolve()
    elif argv:
        print(__doc__.split("Usage:")[1].strip(), file=sys.stderr)
        return 2
    found = violations(root)
    if found:
        print("\n".join(found))
        print(f"lint-capability-cfg: FAIL — {len(found)} capability cfg(s) off a module declaration")
        return 1
    print("lint-capability-cfg: OK — capability cfg sits only on module declarations and the facade")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
