#!/usr/bin/env python3
"""Affected-set selector: given changed files, what must be gated.

Changed files come from `--base SHA [--head SHA]` (a rename counts on both
sides), `--files PATH...`, or stdin (one path per line). Each file is assigned
to the package owning it (nearest ancestor package manifest, across every
workspace in the repository). The production reverse closure over normal and
build edges gives the affected crates; the test-workspace crates reaching an
affected crate directly, and every test crate reaching one of those, give the
test packages, ignoring all edges into and out of the hakari crate. Test
packages map onto the shard groups and archive sets of scripts/test-shard.sh.

A path matching the `[full]` section of scripts/affected-force-full.txt, or
owned by no package and matched by no rule, makes the result `full`. Paths in
the `[none]` section have no gate consequence.

`--check` verifies that scripts/units.txt names exactly the packages outside
the test workspace, each with a valid unit.
"""
import argparse
from collections import defaultdict
import json
from pathlib import Path
import re
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cargo_graph import PRODUCTION_KINDS, repo_graph

ROOT = Path(__file__).resolve().parents[1]
FORCE_FULL = ROOT / "scripts" / "affected-force-full.txt"
UNITS = ROOT / "scripts" / "units.txt"
TEST_SHARD = ROOT / "scripts" / "test-shard.sh"

ROOT_WORKSPACE = "."
TEST_WORKSPACE = "crates/tests"
BRIDGE_WORKSPACE = "bin/bridge"
HACK_CRATE = "systemprompt-test-hack"
UNIT_PATTERN = re.compile(r"^(kernel|tooling|capability:[a-z][a-z0-9-]*)$")


class SelectorError(Exception):
    pass


def parse_rules(text):
    rules = {"full": [], "none": []}
    section = None
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1]
            if section not in rules:
                raise SelectorError(f"line {number}: unknown section [{section}]")
            continue
        if section is None:
            raise SelectorError(f"line {number}: pattern {line!r} outside a section")
        rules[section].append(line)
    return rules


def glob_regex(pattern):
    out, i = [], 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        elif pattern[i] == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile("^" + "".join(out) + "$")


def first_match(path, patterns):
    for pattern in patterns:
        if glob_regex(pattern).match(path):
            return pattern
    return None


def owning_package(path, package_dirs):
    parts = Path(path).parts
    for cut in range(len(parts) - 1, -1, -1):
        candidate = "/".join(parts[:cut]) if cut else "."
        if candidate in package_dirs:
            return candidate
    return None


def parse_units(text):
    units = {}
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        fields = line.split()
        if len(fields) != 2 or not UNIT_PATTERN.match(fields[1]):
            raise SelectorError(f"units line {number}: expected `<package> kernel|tooling|capability:<name>`")
        if fields[0] in units:
            raise SelectorError(f"units line {number}: {fields[0]} listed twice")
        units[fields[0]] = fields[1]
    return units


def check_units(graph, units):
    """Problems with the unit table against the graph; empty when it is complete."""
    expected = {node["name"] for node in graph.values() if node["workspace"] != TEST_WORKSPACE}
    problems = [f"{name}: no unit in scripts/units.txt" for name in sorted(expected - set(units))]
    problems += [f"{name}: listed in scripts/units.txt but no such package" for name in sorted(set(units) - expected)]
    return problems


def reverse_closure(graph, seeds, kinds=None, ignore=frozenset()):
    """Seeds plus every package reaching one of them, over edges of the given kinds."""
    dependents = defaultdict(set)
    for directory, node in graph.items():
        if node["name"] in ignore:
            continue
        for dep, kind in node["deps"]:
            if kinds is not None and kind not in kinds:
                continue
            if dep not in graph or graph[dep]["name"] in ignore:
                continue
            dependents[dep].add(directory)
    seen = set(seeds)
    frontier = list(seeds)
    while frontier:
        for dependent in dependents[frontier.pop()]:
            if dependent not in seen:
                seen.add(dependent)
                frontier.append(dependent)
    return seen


def shard_groups_for(test_dirs, shard_map):
    groups = []
    for group, prefixes in shard_map["groups"]:
        if any(p in f"/{d}/Cargo.toml" for d in test_dirs for p in prefixes):
            groups.append(group)
    return groups


def select(changed, graph, rules, shard_map, units):
    changed = sorted({p.strip().removeprefix("./") for p in changed} - {""})
    production = {d for d, n in graph.items() if n["workspace"] != TEST_WORKSPACE}
    tests = {d for d, n in graph.items() if n["workspace"] == TEST_WORKSPACE and n["name"] != HACK_CRATE}
    reasons, owners = [], set()
    for path in changed:
        pattern = first_match(path, rules["full"])
        if pattern:
            reasons.append(f"{path}: matches force-full pattern {pattern}")
            continue
        owner = owning_package(path, graph)
        if owner is not None:
            owners.add(owner)
        elif first_match(path, rules["none"]) is None:
            reasons.append(f"{path}: owned by no crate and matched by no rule")

    def name(dirs):
        return sorted(graph[d]["name"] for d in dirs)

    changed_prod = owners & production
    unit_names = sorted({units.get(graph[d]["name"], "unclassified") for d in changed_prod})
    has_kernel = "kernel" in unit_names
    has_capability = any(u.startswith("capability:") for u in unit_names)
    mixed = (has_kernel and has_capability) or "unclassified" in unit_names

    if reasons:
        mode = "full"
        affected = set(production)
        test_set = set(tests)
    else:
        affected = reverse_closure({d: graph[d] for d in production}, changed_prod, kinds=PRODUCTION_KINDS)
        seeds = (owners & tests) | {
            d for d in tests if any(dep in affected for dep, _ in graph[d]["deps"])
        }
        test_graph = {d: graph[d] for d in graph if graph[d]["workspace"] == TEST_WORKSPACE}
        test_set = reverse_closure(test_graph, seeds, ignore={HACK_CRATE}) & tests
        mode = "affected" if owners else "none"

    groups = shard_groups_for(test_set, shard_map)
    archive_sets = sorted({shard_map["archive_of"][g] for g in groups})
    return {
        "mode": mode,
        "reasons": reasons,
        "changed_crates": name(owners),
        "affected_crates": name(affected),
        "clippy_packages": name(d for d in affected if graph[d]["workspace"] == ROOT_WORKSPACE),
        "test_packages": name(test_set),
        "shard_groups": groups,
        "archive_sets": archive_sets,
        "bridge": any(graph[d]["workspace"] == BRIDGE_WORKSPACE for d in affected),
        "units": unit_names,
        "mixed_kernel_and_capability": mixed,
        "scope_size": len(affected) + len(test_set),
    }


def load_shard_map(script=TEST_SHARD):
    def run(*args):
        return subprocess.run(["bash", str(script), *args], check=True, capture_output=True, text=True).stdout

    text = Path(script).read_text()
    match = re.search(r"^group_prefixes\(\) \{\n.*?^\}\n", text, re.S | re.M)
    if not match:
        raise SelectorError(f"{script}: no group_prefixes() function")
    groups = []
    for group in run("--list").split():
        prefixes = subprocess.run(
            ["bash", "-c", match.group(0) + 'group_prefixes "$1"', "_", group],
            check=True, capture_output=True, text=True,
        ).stdout.split()
        groups.append((group, prefixes))
    archive_of = {group: run("--archive-set-of", group).strip() for group, _ in groups}
    return {"groups": groups, "archive_of": archive_of}


def changed_files(base, head):
    out = subprocess.run(
        ["git", "diff", "--name-only", "--no-renames", base, head],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return out.splitlines()


def render(result, fmt):
    if fmt == "json":
        return json.dumps(result, indent=2)
    if fmt == "github":
        lines = []
        for key, value in result.items():
            if isinstance(value, bool):
                value = "true" if value else "false"
            elif isinstance(value, list):
                value = json.dumps(value, separators=(",", ":"))
            lines.append(f"{key}={value}")
        return "\n".join(lines)
    lines = [f"mode: {result['mode']} (scope {result['scope_size']})"]
    for key, value in result.items():
        if isinstance(value, list):
            lines.append(f"{key} ({len(value)}):")
            lines.extend(f"  {v}" for v in value)
        elif key not in ("mode", "scope_size"):
            lines.append(f"{key}: {str(value).lower()}")
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--base")
    source.add_argument("--files", nargs="*")
    source.add_argument("--check", action="store_true")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--format", choices=("json", "github", "text"), default="json")
    args = parser.parse_args(argv)
    try:
        graph = repo_graph(ROOT)
        units = parse_units(UNITS.read_text())
        if args.check:
            problems = check_units(graph, units)
            for problem in problems:
                print(f"  {problem}")
            print(f"affected: {'FAIL' if problems else 'OK'} — {len(units)} units listed")
            return 1 if problems else 0
        if args.base:
            changed = changed_files(args.base, args.head)
        elif args.files is not None:
            changed = args.files
        else:
            changed = sys.stdin.read().splitlines()
        result = select(changed, graph, parse_rules(FORCE_FULL.read_text()), load_shard_map(), units)
    except (SelectorError, subprocess.CalledProcessError) as error:
        print(f"affected: {error}", file=sys.stderr)
        return 2
    print(render(result, args.format))
    return 0


if __name__ == "__main__":
    sys.exit(main())
