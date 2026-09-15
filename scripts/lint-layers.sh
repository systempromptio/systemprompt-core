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
#      layers. LEGACY_DOMAIN_EDGES below is empty by design: every edge fails
#      the gate, and adding one requires a written justification in the
#      commit that adds it.
#   4. The shared layer carries no I/O capability. A `crates/shared/*` manifest
#      may not list `reqwest`, `axum`, `libc`, a non-optional `sqlx`, or
#      `tokio` with the `net` (or `full`) feature — the workspace `tokio`
#      definition carries `net`, so a bare `workspace = true` inherits it.
#      `systemprompt-client` is the one sanctioned network crate
#      (architecture.md "The client crate performs network I/O") and is
#      exempt from the `reqwest` / `tokio` rows only. `axum` is exempt for
#      exactly two crates: `systemprompt-extension` (the extension routing
#      contract is `ApiExtensionTyped::router() -> axum::Router`) and
#      `systemprompt-models` behind its optional `web` feature (the
#      `IntoResponse` impls for the API envelopes). Neither opens a socket;
#      replacing them is a router-abstraction redesign, not a dependency trim.
#
# Layer membership is read from each crate's position on disk (crates/<layer>/),
# so a crate moved between layers is re-classified automatically. Only normal
# and build dependencies are considered: dev-dependencies may legitimately point
# at test helpers in any layer and are not part of the shipped graph.
#
# Table ownership (a crate querying tables another crate's schema declares) is
# the SQL face of the same boundary and is gated by lint-table-ownership.sh.
#
# These dependency properties are checked statically. They are cheap
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

NETWORK_CRATE = "systemprompt-client"
ROUTER_CONTRACT_CRATES = {"systemprompt-extension", "systemprompt-models"}
capability = []
for name in sorted(local):
    if layer[name] != "shared":
        continue
    for d in pkgs[name]["dependencies"]:
        if d["kind"] not in (None, "build"):
            continue
        dep = d["name"]
        if dep in ("reqwest", "tokio") and name == NETWORK_CRATE:
            continue
        if dep == "axum" and name in ROUTER_CONTRACT_CRATES:
            continue
        if dep in ("reqwest", "axum", "libc"):
            optional = " (optional)" if d["optional"] else ""
            capability.append(f"  {name} -> {dep}{optional}: shared crates carry no I/O capability")
        elif dep == "sqlx" and not d["optional"]:
            capability.append(f"  {name} -> sqlx (non-optional): shared crates carry no SQL")
        elif dep == "tokio" and ({"net", "full"} & set(d["features"])):
            capability.append(f"  {name} -> tokio[net]: shared crates open no sockets (inherited from the workspace tokio features)")

if violations:
    print("Dependencies pointing upward through the layer stack:")
    print("\n".join(violations))
if cycles:
    print("Dependency cycles:")
    for c in cycles:
        print(f"  {c}")
if capability:
    print("Shared-layer manifests listing an I/O capability:")
    print("\n".join(capability))

if violations or cycles or capability:
    print(f"lint-layers: FAIL — {len(violations)} layer violation(s), {len(cycles)} cycle(s), {len(capability)} shared-layer capability dep(s)")
    sys.exit(1)

print(f"lint-layers: OK — {len(local)} crates, no upward dependencies, no cycles, domain isolation holds, shared layer is I/O-free")
'

if rg --line-number --ignore-case --multiline \
    '\b(FROM|JOIN|UPDATE|INSERT\s+INTO|DELETE\s+FROM)\s+(public\.)?(users|user_sessions|agent_tasks|task_messages|user_contexts|ai_requests|ai_request_messages|mcp_tool_executions|markdown_content|logs|analytics_events)\b' \
    crates/domain/analytics/src; then
    echo 'lint-layers: FAIL — analytics SQL must use its reporting tables or owner traits'
    exit 1
fi
