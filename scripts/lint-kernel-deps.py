#!/usr/bin/env python3
"""Keep the kernel crates free of the capabilities they compose.

The kernel crates (`systemprompt-runtime`, `-api`, `-cli`, `-scheduler`,
`-mcp-host`) mount, dispatch and resolve whatever a composition admits; they
never name a capability. So none of them may depend on a domain crate
(`crates/domain/*`) or a capability entry crate (`systemprompt-*-entry`).
A capability entry crate in turn contributes through the extension
contracts, so it may not depend on `systemprompt-cli` or `systemprompt-api`.

Only normal and build dependencies count, as in lint-layers. The graph is
`cargo metadata --offline --no-deps` of the root workspace.

Usage: lint-kernel-deps.py [--metadata FILE]
  --metadata FILE   read the cargo metadata JSON from FILE (fixture graphs)
"""
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from cargo_graph import load_metadata, packages_by_name, production_edges  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
KERNEL = (
    "systemprompt-runtime",
    "systemprompt-api",
    "systemprompt-cli",
    "systemprompt-scheduler",
    "systemprompt-mcp-host",
)
ENTRY_FORBIDDEN = ("systemprompt-cli", "systemprompt-api")


def layer_of(pkg):
    parts = pkg["manifest_path"].split("/crates/")
    return parts[1].split("/")[0] if len(parts) > 1 else "facade"


def is_entry_capability(name):
    return name.startswith("systemprompt-") and name.endswith("-entry")


def violations(metadata):
    pkgs = packages_by_name(metadata)
    deps = production_edges(pkgs)
    found = []
    for name in KERNEL:
        for dep in sorted(deps.get(name, ())):
            if layer_of(pkgs[dep]) == "domain":
                found.append(f"  {name} -> {dep}: a kernel crate depends on no domain crate")
            elif is_entry_capability(dep):
                found.append(f"  {name} -> {dep}: a kernel crate depends on no capability entry crate")
    for name in sorted(pkgs):
        if not is_entry_capability(name):
            continue
        for dep in sorted(deps.get(name, set()) & set(ENTRY_FORBIDDEN)):
            found.append(f"  {name} -> {dep}: a capability entry crate contributes through systemprompt-extension")
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
        print(f"lint-kernel-deps: FAIL — {len(found)} kernel or entry dependency violation(s)")
        return 1
    print("lint-kernel-deps: OK — kernel crates name no domain or entry crate, entry crates name no kernel entry")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
