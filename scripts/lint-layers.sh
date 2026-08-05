#!/usr/bin/env bash
# Pre-merge gate enforcing the layered dependency rule from CLAUDE.md:
#
#   entry -> app -> domain -> infra -> shared
#
# Two properties are checked over the main workspace's crate graph, both of
# which must hold exactly (there is no threshold to tune):
#
#   1. No dependency points upward. A crate may depend on its own layer or any
#      layer below it, never above. The `systemprompt` facade sits above entry
#      and may depend on anything.
#   2. No dependency cycles.
#   3. No domain -> domain dependencies. Domain crates are peers: cross-domain
#      capability flows through shared-layer traits (DynAiProvider,
#      ToolProvider, provider-contracts), wired at app/entry composition
#      layers. LEGACY_DOMAIN_EDGES below allowlists the edges that predate the
#      rule and are being removed; deleting an edge deletes its entry, and any
#      edge not in the list fails the gate.
#
# Layer membership is read from each crate's position on disk (crates/<layer>/),
# so a crate moved between layers is re-classified automatically. Only normal
# and build dependencies are considered: dev-dependencies may legitimately point
# at test helpers in any layer and are not part of the shipped graph.
#
# Both properties were previously conventions enforced by review. They are cheap
# and deterministic, so they are enforced here instead.

set -euo pipefail

cd "$(dirname "$0")/.."

command -v python3 >/dev/null || { echo "lint-layers: python3 not found"; exit 1; }

cargo metadata --no-deps --format-version 1 | python3 -c '
import json, sys
from collections import defaultdict

ORDER = {"shared": 0, "infra": 1, "domain": 2, "app": 3, "entry": 4, "facade": 5}

md = json.load(sys.stdin)
pkgs = {p["name"]: p for p in md["packages"]}
local = set(pkgs)

layer = {}
for name, pkg in pkgs.items():
    parts = pkg["manifest_path"].split("/crates/")
    layer[name] = parts[1].split("/")[0] if len(parts) > 1 else "facade"

unknown = sorted(n for n, l in layer.items() if l not in ORDER)
if unknown:
    for n in unknown:
        print(f"  {n}: unrecognised layer {layer[n]!r}")
    print("lint-layers: FAIL — crate outside the known layer taxonomy")
    sys.exit(1)

deps = defaultdict(set)
for name, pkg in pkgs.items():
    for d in pkg["dependencies"]:
        if d["name"] in local and d["name"] != name and d["kind"] in (None, "build"):
            deps[name].add(d["name"])

# Empty by design: adding an edge here requires a written justification in the
# commit that adds it. Cross-domain capability flows through shared-layer traits.
LEGACY_DOMAIN_EDGES = set()

violations = []
for name in sorted(local):
    for dep in sorted(deps[name]):
        if ORDER[layer[dep]] > ORDER[layer[name]]:
            violations.append(f"  {name} ({layer[name]}) -> {dep} ({layer[dep]})")
        elif (
            layer[name] == "domain"
            and layer[dep] == "domain"
            and (name, dep) not in LEGACY_DOMAIN_EDGES
        ):
            violations.append(f"  {name} (domain) -> {dep} (domain): domain crates must not depend on each other")

WHITE, GREY, BLACK = 0, 1, 2
colour = defaultdict(int)
stack = []
cycles = []

def visit(node):
    colour[node] = GREY
    stack.append(node)
    for dep in sorted(deps[node]):
        if colour[dep] == GREY:
            cycles.append(" -> ".join(stack[stack.index(dep):] + [dep]))
        elif colour[dep] == WHITE:
            visit(dep)
    stack.pop()
    colour[node] = BLACK

for name in sorted(local):
    if colour[name] == WHITE:
        visit(name)

if violations:
    print("Dependencies pointing upward through the layer stack:")
    print("\n".join(violations))
if cycles:
    print("Dependency cycles:")
    for c in cycles:
        print(f"  {c}")

if violations or cycles:
    print(f"lint-layers: FAIL — {len(violations)} layer violation(s), {len(cycles)} cycle(s)")
    sys.exit(1)

print(f"lint-layers: OK — {len(local)} crates, no upward dependencies, no cycles, domain isolation holds")
'
