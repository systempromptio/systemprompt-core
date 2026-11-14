"""Cargo package graph loader shared by the dependency gates, the affected-set
selector and the build report.

`cargo metadata --no-deps` is the input of the repository graph. Production
edges are normal and build dependencies; dev-dependencies may point at test
helpers anywhere and are not part of the shipped graph. `resolved_closure`
reads a full `cargo metadata` resolve instead (the build report's crate
closure):

    cargo_graph.py closure < metadata.json   prints `<packages> <path packages>`
"""
from collections import defaultdict
import json
import os
from pathlib import Path
import subprocess
import sys

PRODUCTION_KINDS = (None, "build")


def load_metadata(manifest_path, offline=True):
    command = ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest_path)]
    if offline:
        command.append("--offline")
    return json.loads(subprocess.run(command, check=True, capture_output=True, text=True).stdout)


def packages_by_name(metadata):
    return {p["name"]: p for p in metadata["packages"]}


def production_edges(pkgs):
    """Name-keyed normal + build edges between the given packages."""
    local = set(pkgs)
    deps = defaultdict(set)
    for name, pkg in pkgs.items():
        for d in pkg["dependencies"]:
            if d["name"] in local and d["name"] != name and d["kind"] in PRODUCTION_KINDS:
                deps[name].add(d["name"])
    return deps


def workspace_manifests(root):
    """Every tracked Cargo.toml that declares a `[workspace]`, as repo-relative paths."""
    listed = subprocess.run(
        ["git", "ls-files", "--", "Cargo.toml", "**/Cargo.toml"],
        cwd=root, check=True, capture_output=True, text=True,
    ).stdout.split()
    found = []
    for rel in sorted(listed):
        text = (Path(root) / rel).read_text()
        if any(line.strip() == "[workspace]" for line in text.splitlines()):
            found.append(rel)
    return found


def repo_graph(root, offline=True):
    """All packages of every workspace in the repository, keyed by package directory.

    Returns `{dir: {"name", "workspace", "deps": [(dep_dir, kind)]}}` with
    repo-relative directories; `workspace` is the repo-relative directory of the
    owning workspace root. Only path dependencies that land on a known package
    become edges.
    """
    root = Path(root).resolve()
    raw = []
    for manifest in workspace_manifests(root):
        metadata = load_metadata(root / manifest, offline=offline)
        workspace = os.path.relpath(metadata["workspace_root"], root)
        raw.extend((workspace, pkg) for pkg in metadata["packages"])
    graph = {}
    for workspace, pkg in raw:
        directory = os.path.relpath(os.path.dirname(pkg["manifest_path"]), root)
        graph[directory] = {"name": pkg["name"], "workspace": workspace, "deps": []}
    for workspace, pkg in raw:
        directory = os.path.relpath(os.path.dirname(pkg["manifest_path"]), root)
        for d in pkg["dependencies"]:
            if not d.get("path"):
                continue
            target = os.path.relpath(d["path"], root)
            if target in graph and target != directory:
                graph[directory]["deps"].append((target, d["kind"]))
    return graph


def resolved_closure(metadata):
    """Package ids the root package reaches over normal and build edges of the
    resolve, excluding the root itself.

    Run `cargo metadata --filter-platform <host>` to get the host-target closure
    `cargo tree -e normal,build` prints; without it the resolve spans every
    target. Every count comes from this one resolve, so a run with and without
    `--offline` over the same lock agrees.
    """
    resolve = metadata.get("resolve") or {}
    root = resolve.get("root")
    if root is None:
        raise ValueError("cargo metadata has no resolve root; run it inside a package")
    nodes = {node["id"]: node for node in resolve["nodes"]}
    seen, stack = set(), [root]
    while stack:
        for dep in nodes[stack.pop()].get("deps", []):
            kinds = {k["kind"] for k in dep.get("dep_kinds", [])}
            if kinds & set(PRODUCTION_KINDS) and dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                stack.append(dep["pkg"])
    seen.discard(root)
    return seen


def path_packages(metadata, ids):
    """The ids among `ids` that are path packages (no registry or git source)."""
    source = {p["id"]: p.get("source") for p in metadata["packages"]}
    return {i for i in ids if source.get(i) is None}


if __name__ == "__main__":
    if sys.argv[1:] != ["closure"]:
        print("usage: cargo_graph.py closure < metadata.json", file=sys.stderr)
        sys.exit(2)
    loaded = json.load(sys.stdin)
    closure = resolved_closure(loaded)
    print(len(closure), len(path_packages(loaded, closure)))
