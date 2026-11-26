#!/usr/bin/env python3
"""The kernel floor gate judges transitive closures over fixture graphs.

`scripts/lint-kernel-deps.py --metadata` reads a cargo metadata fixture; these
tests pin the rule: a capability at any depth fails a kernel crate, optional
edges count, dev edges do not, the facade counts only the optional edges its
`floor` features enable, and the shortest path is printed.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-kernel-deps.py"

UNITS = """
systemprompt-runtime kernel
systemprompt-api kernel
systemprompt-models kernel
systemprompt kernel
systemprompt-gateway capability:gateway
systemprompt-ai capability:gateway
systemprompt-files capability:files
systemprompt-files-entry capability:files
"""
COMPOSITIONS = "runtime core,runtime boot,floor\nreference reference boot\n"


def dep(name, kind=None, optional=False):
    return {"name": name, "kind": kind, "optional": optional}


def package(name, deps=(), features=None):
    return {"name": name, "manifest_path": f"/r/crates/x/{name}/Cargo.toml",
            "dependencies": list(deps), "features": features or {}}


def graph(*packages):
    names = {p["name"] for p in packages}
    every = list(packages) + [package(n) for n in UNITS.split()[::2] if n not in names]
    return {"packages": every}


def run(metadata, compositions=COMPOSITIONS):
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        (tmp / "m.json").write_text(json.dumps(metadata))
        (tmp / "units.txt").write_text(UNITS)
        (tmp / "c.txt").write_text(compositions)
        result = subprocess.run(
            [sys.executable, "-B", str(SCRIPT), "--metadata", str(tmp / "m.json"),
             "--units", str(tmp / "units.txt"), "--compositions", str(tmp / "c.txt")],
            capture_output=True, text=True)
    return result.returncode, result.stdout + result.stderr


class KernelFloor(unittest.TestCase):
    def test_clean_graph_passes(self):
        code, out = run(graph(package("systemprompt-runtime", [dep("systemprompt-models")])))
        self.assertEqual(code, 0, out)
        self.assertIn("lint-kernel-deps: OK", out)

    def test_direct_edge_fails(self):
        code, out = run(graph(package("systemprompt-api", [dep("systemprompt-gateway")])))
        self.assertEqual(code, 1, out)
        self.assertIn("systemprompt-api -> systemprompt-gateway:", out)

    def test_one_hop_fails(self):
        code, out = run(graph(
            package("systemprompt-api", [dep("systemprompt-runtime")]),
            package("systemprompt-runtime", [dep("systemprompt-files")]),
        ))
        self.assertEqual(code, 1, out)
        self.assertIn("systemprompt-api -> systemprompt-runtime -> systemprompt-files:", out)

    def test_two_hop_through_a_capability_names_both(self):
        code, out = run(graph(
            package("systemprompt-api", [dep("systemprompt-runtime")]),
            package("systemprompt-runtime", [dep("systemprompt-gateway")]),
            package("systemprompt-gateway", [dep("systemprompt-ai")]),
        ))
        self.assertEqual(code, 1, out)
        self.assertIn("systemprompt-api -> systemprompt-runtime -> systemprompt-gateway -> systemprompt-ai:", out)

    def test_optional_edge_of_a_kernel_crate_counts(self):
        code, out = run(graph(package("systemprompt-runtime", [dep("systemprompt-files", optional=True)])))
        self.assertEqual(code, 1, out)
        self.assertIn("systemprompt-runtime -> systemprompt-files:", out)

    def test_dev_dependency_is_ignored(self):
        code, out = run(graph(package("systemprompt-runtime", [dep("systemprompt-files", kind="dev")])))
        self.assertEqual(code, 0, out)

    def test_shortest_path_is_printed(self):
        code, out = run(graph(
            package("systemprompt-api", [dep("systemprompt-runtime"), dep("systemprompt-gateway")]),
            package("systemprompt-runtime", [dep("systemprompt-models")]),
            package("systemprompt-models", [dep("systemprompt-gateway")]),
        ))
        self.assertEqual(code, 1, out)
        self.assertIn("  systemprompt-api -> systemprompt-gateway: a kernel crate links no capability"
                      " (systemprompt-gateway is capability:gateway)", out)
        self.assertNotIn("systemprompt-api -> systemprompt-runtime -> systemprompt-models -> systemprompt-gateway", out)

    def test_facade_optional_capability_outside_the_floor_features_passes(self):
        facade = package("systemprompt", [dep("systemprompt-runtime", optional=True),
                                          dep("systemprompt-files-entry", optional=True)],
                         {"default": ["core"], "core": [], "runtime": ["dep:systemprompt-runtime"],
                          "files": ["dep:systemprompt-files-entry"],
                          "api": ["systemprompt-files-entry?/http"]})
        code, out = run(graph(facade))
        self.assertEqual(code, 0, out)

    def test_facade_capability_enabled_by_the_floor_features_fails(self):
        facade = package("systemprompt", [dep("systemprompt-files-entry", optional=True)],
                         {"default": [], "runtime": ["api"], "api": ["systemprompt-files-entry/http"]})
        code, out = run(graph(facade))
        self.assertEqual(code, 1, out)
        self.assertIn("systemprompt -> systemprompt-files-entry:", out)

    def test_facade_non_optional_capability_fails(self):
        code, out = run(graph(package("systemprompt", [dep("systemprompt-gateway")])))
        self.assertEqual(code, 1, out)

    def test_every_kernel_row_is_judged(self):
        code, out = run(graph(package("systemprompt-models", [dep("systemprompt-ai")])))
        self.assertEqual(code, 1, out)
        self.assertIn("systemprompt-models -> systemprompt-ai:", out)

    def test_a_capability_may_link_another_capability_package_of_its_unit(self):
        code, out = run(graph(package("systemprompt-gateway", [dep("systemprompt-ai"), dep("systemprompt-models")])))
        self.assertEqual(code, 0, out)

    def test_entry_crate_may_not_name_the_api(self):
        code, out = run(graph(package("systemprompt-files-entry", [dep("systemprompt-api")])))
        self.assertEqual(code, 1, out)
        self.assertIn("systemprompt-files-entry -> systemprompt-api: a capability entry crate", out)

    def test_composition_table_needs_one_floor_line(self):
        code, out = run(graph(), compositions="runtime core,runtime boot\n")
        self.assertEqual(code, 2, out)
        self.assertIn("exactly one line flagged floor", out)


if __name__ == "__main__":
    unittest.main()
