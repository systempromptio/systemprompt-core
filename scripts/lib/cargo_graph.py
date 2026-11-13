"""Cargo package graph loader shared by the dependency gates and the affected-set selector.

`cargo metadata --no-deps` is the only input. Production edges are normal and
build dependencies; dev-dependencies may point at test helpers anywhere and are
not part of the shipped graph.
"""
from collections import defaultdict
import json
import os
from pathlib import Path
import subprocess

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
