"""Cargo package graph loader shared by the dependency gates, the affected-set
selector and the build report.

`cargo metadata --no-deps` is the input of the repository graph. Production
edges are normal and build dependencies; dev-dependencies may point at test
helpers anywhere and are not part of the shipped graph. The build report's
crate closure is read from `cargo tree -e normal,build --prefix none`
(`tree_closure`), which is feature-resolved per dependency kind and target as
a build is; a `cargo metadata` resolve is not:

    cargo_graph.py tree-closure < tree.txt   prints `<packages> <path packages>`
    cargo_graph.py tree-capabilities UNITS < tree.txt
                                             prints the capability-unit path
                                             packages of the closure, one per line
"""
from collections import defaultdict
import json
import os
import re
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


def capability_edges(metadata, units):
    """Sorted `(package, dependency)` production edges between packages of two
    different capabilities (`capability:<name>` rows of the unit table)."""
    deps = production_edges(packages_by_name(metadata))
    found = []
    for name in sorted(deps):
        unit = units.get(name, "")
        if not unit.startswith("capability:"):
            continue
        for dep in sorted(deps[name]):
            other = units.get(dep, "")
            if other.startswith("capability:") and other != unit:
                found.append((name, dep))
    return found


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


def tree_closure(text):
    """`{(name, version): path-or-None}` for every package `cargo tree --prefix
    none` prints, excluding the root (its first line)."""
    found = {}
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    for line in lines[1:]:
        words = line.split()
        if len(words) < 2 or not words[1].startswith("v"):
            continue
        groups = re.findall(r"\(([^)]*)\)", line)
        path = next((g for g in groups if g.startswith("/")), None)
        found[(words[0], words[1][1:])] = path
    return found


def tree_capabilities(closure, units):
    """Sorted names of the path packages of a `tree_closure` whose unit is
    `capability:*`."""
    return sorted({
        name for (name, _), path in closure.items()
        if path is not None and units.get(name, "").startswith("capability:")
    })


if __name__ == "__main__":
    args = sys.argv[1:]
    if args == ["tree-closure"]:
        closure = tree_closure(sys.stdin.read())
        print(len(closure), sum(1 for path in closure.values() if path is not None))
    elif len(args) == 2 and args[0] == "tree-capabilities":
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from units import load_units
        for package in tree_capabilities(tree_closure(sys.stdin.read()), load_units(args[1])):
            print(package)
    else:
        print("usage: cargo_graph.py tree-closure | tree-capabilities UNITS < tree.txt", file=sys.stderr)
        sys.exit(2)
