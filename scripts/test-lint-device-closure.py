#!/usr/bin/env python3
"""lint-device-closure refuses exactly the edges that join the kernel and device closures above shared.

Each test writes a cargo metadata document and a unit table for a fixture
graph and runs the gate over them with `--metadata` and `--units`. The last
test holds the gate's device list equal to the layer table of lint-layers.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-device-closure.py"
LAYERS = ROOT / "scripts" / "lint-layers.sh"

DEVICE_LAYERS = {
    "systemprompt-device": "infra",
    "systemprompt-machine": "infra",
    "systemprompt-engine": "domain",
    "systemprompt-proxy": "domain",
    "systemprompt-hosts": "domain",
    "systemprompt-drive": "domain",
    "systemprompt-runner": "domain",
    "systemprompt-harness": "app",
    "systemprompt-mobile": "app",
}
SERVER = {
    "systemprompt-identifiers": ("shared", "kernel", ()),
    "systemprompt-models": ("shared", "kernel", ("systemprompt-identifiers",)),
    "systemprompt-database": ("infra", "kernel", ("systemprompt-models",)),
    "systemprompt-users": ("domain", "kernel", ("systemprompt-database",)),
    "systemprompt-runtime": ("app", "kernel", ("systemprompt-users",)),
    "systemprompt-api": ("entry", "kernel", ("systemprompt-runtime",)),
    "systemprompt-agent-runs": ("app", "capability:agents", ("systemprompt-database",)),
}


def package(name, layer, deps=(), kind=None):
    path = f"/fixture/crates/{layer}/{name}/Cargo.toml" if layer != "facade" else f"/fixture/{name}/Cargo.toml"
    return {
        "name": name,
        "manifest_path": path,
        "dependencies": [{"name": d, "kind": kind, "optional": False, "features": []} for d in deps],
    }


def graph(edges=None, without=(), dev_edges=None):
    """The fixture workspace; `edges` adds production dependencies, `dev_edges` dev ones, `without` drops crates."""
    edges = edges or {}
    packages = []
    for name, layer in DEVICE_LAYERS.items():
        packages.append(package(name, layer, edges.get(name, ())))
    for name, (layer, _, deps) in SERVER.items():
        packages.append(package(name, layer, (*deps, *edges.get(name, ()))))
    packages.append(package("systemprompt", "facade", ("systemprompt-api", *edges.get("systemprompt", ()))))
    for name, deps in (dev_edges or {}).items():
        target = next(p for p in packages if p["name"] == name)
        target["dependencies"] += package(name, "x", deps, kind="dev")["dependencies"]
    return [p for p in packages if p["name"] not in without]


def units(overrides=None):
    table = {name: "capability:device" for name in DEVICE_LAYERS}
    table |= {name: unit for name, (_, unit, _) in SERVER.items()}
    table["systemprompt"] = "kernel"
    table |= overrides or {}
    return "".join(f"{name} {unit}\n" for name, unit in table.items())


class DeviceClosure(unittest.TestCase):
    def run_gate(self, packages, unit_text):
        paths = []
        for suffix, text in ((".json", json.dumps({"packages": packages})), (".txt", unit_text)):
            with tempfile.NamedTemporaryFile("w", suffix=suffix, delete=False) as out:
                out.write(text)
            self.addCleanup(os.unlink, out.name)
            paths.append(out.name)
        return subprocess.run(
            ["python3", str(SCRIPT), "--metadata", paths[0], "--units", paths[1]],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def check(self, packages, expected, unit_text=None):
        result = self.run_gate(packages, units() if unit_text is None else unit_text)
        lines = sorted(line.split(":")[0].strip() for line in result.stdout.splitlines() if line.startswith("  "))
        self.assertEqual(lines, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_empty_device_crates_pass(self):
        self.check(graph(), [])

    def test_device_crates_on_each_other_and_shared_pass(self):
        self.check(graph({
            "systemprompt-device": ["systemprompt-models"],
            "systemprompt-machine": ["systemprompt-device"],
            "systemprompt-engine": ["systemprompt-device", "systemprompt-identifiers"],
            "systemprompt-harness": [name for name in DEVICE_LAYERS if name != "systemprompt-harness"],
        }), [])

    def test_the_closures_meet_in_shared(self):
        result = self.run_gate(graph({"systemprompt-device": ["systemprompt-models"]}), units())
        self.assertIn("meet in 2 shared crate(s)", result.stdout)

    def test_every_kernel_crate_naming_a_device_crate_fails(self):
        kernel = [name for name, (_, unit, _) in SERVER.items() if unit == "kernel"]
        for name in kernel:
            with self.subTest(name=name):
                self.check(graph({name: ["systemprompt-device"]}), [f"{name} -> systemprompt-device"])

    def test_every_device_crate_is_refused_to_the_kernel(self):
        for name in DEVICE_LAYERS:
            with self.subTest(name=name):
                self.check(graph({"systemprompt-runtime": [name]}), [f"systemprompt-runtime -> {name}"])

    def test_kernel_reaching_a_device_crate_through_a_capability_fails(self):
        self.check(
            graph({
                "systemprompt-runtime": ["systemprompt-agent-runs"],
                "systemprompt-agent-runs": ["systemprompt-runner"],
            }),
            ["systemprompt-agent-runs -> systemprompt-runner"],
        )

    def test_capability_outside_the_kernel_closure_may_name_a_device_crate(self):
        self.check(graph({"systemprompt-agent-runs": ["systemprompt-runner"]}), [])

    def test_facade_may_name_device_crates(self):
        self.check(graph({"systemprompt": list(DEVICE_LAYERS)}), [])

    def test_every_device_crate_naming_a_server_crate_above_shared_fails(self):
        for name in DEVICE_LAYERS:
            with self.subTest(name=name):
                self.check(graph({name: ["systemprompt-database"]}), [f"{name} -> systemprompt-database"])

    def test_device_crate_naming_each_server_layer_and_the_facade_fails(self):
        targets = ["systemprompt-database", "systemprompt-users", "systemprompt-agent-runs", "systemprompt-api", "systemprompt"]
        self.check(
            graph({"systemprompt-harness": targets}),
            [f"systemprompt-harness -> {target}" for target in targets],
        )

    def test_one_wrong_edge_is_one_line(self):
        self.check(
            graph({"systemprompt-device": ["systemprompt-runtime"], "systemprompt-harness": ["systemprompt-device"]}),
            ["systemprompt-device -> systemprompt-runtime"],
        )

    def test_device_crate_listed_as_kernel_fails(self):
        self.check(graph(), ["systemprompt-proxy"], units({"systemprompt-proxy": "kernel"}))

    def test_missing_device_crate_fails(self):
        for name in DEVICE_LAYERS:
            with self.subTest(name=name):
                self.check(graph(without=(name,)), [name])

    def test_dev_dependency_is_ignored(self):
        self.check(
            graph(dev_edges={
                "systemprompt-runtime": ["systemprompt-device"],
                "systemprompt-engine": ["systemprompt-database"],
            }),
            [],
        )

    def test_device_list_matches_the_layer_gate(self):
        table = re.search(r"^DEVICE_LAYERS = \{\n(.*?)^\}", LAYERS.read_text(), re.S | re.M)
        self.assertIsNotNone(table, "lint-layers.sh has no DEVICE_LAYERS table")
        placed = dict(re.findall(r'"([a-z-]+)": "([a-z]+)"', table.group(1)))
        self.assertEqual(placed, DEVICE_LAYERS)
        gate = re.search(r"^DEVICE = \(\n(.*?)^\)", SCRIPT.read_text(), re.S | re.M)
        self.assertIsNotNone(gate, "lint-device-closure.py has no DEVICE tuple")
        self.assertEqual(sorted(re.findall(r'"([a-z-]+)"', gate.group(1))), sorted(DEVICE_LAYERS))


if __name__ == "__main__":
    unittest.main()
