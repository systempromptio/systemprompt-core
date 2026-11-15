#!/usr/bin/env python3
"""lint-layers judges a crate graph by the layer rules in its header.

Each test writes a cargo metadata document for a fixture graph and runs the
gate over it with `--metadata`. The last test runs the gate over the
repository, which must pass.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-layers.sh"


def dep(name, optional=False, features=(), kind=None):
    return {"name": name, "optional": optional, "features": list(features), "kind": kind}


def package(name, layer, deps=()):
    return {"name": name, "manifest_path": f"/fixture/crates/{layer}/{name}/Cargo.toml", "dependencies": list(deps)}


class LintLayers(unittest.TestCase):
    def run_graph(self, packages):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as out:
            json.dump({"packages": packages}, out)
        self.addCleanup(os.unlink, out.name)
        return subprocess.run(
            ["bash", str(SCRIPT), "--metadata", out.name],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def assert_passes(self, packages):
        result = self.run_graph(packages)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def assert_fails(self, packages, needle):
        result = self.run_graph(packages)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(needle, result.stdout)

    def test_optional_axum_and_clap_in_extension_pass(self):
        self.assert_passes([
            package("systemprompt-extension", "shared", [dep("axum", optional=True), dep("clap", optional=True)]),
        ])

    def test_non_optional_axum_in_extension_fails(self):
        self.assert_fails(
            [package("systemprompt-extension", "shared", [dep("axum"), dep("clap", optional=True)])],
            "systemprompt-extension -> axum (non-optional)",
        )

    def test_non_optional_clap_in_extension_fails(self):
        self.assert_fails(
            [package("systemprompt-extension", "shared", [dep("axum", optional=True), dep("clap")])],
            "systemprompt-extension -> clap (non-optional)",
        )

    def test_axum_in_another_shared_crate_fails(self):
        self.assert_fails(
            [package("systemprompt-traits", "shared", [dep("axum", optional=True)])],
            "systemprompt-traits -> axum (optional)",
        )

    def test_upward_edge_fails(self):
        self.assert_fails(
            [package("systemprompt-traits", "shared", [dep("systemprompt-users")]), package("systemprompt-users", "domain")],
            "systemprompt-traits (shared) -> systemprompt-users (domain)",
        )

    def test_dev_dependency_is_ignored(self):
        self.assert_passes([
            package("systemprompt-traits", "shared", [dep("systemprompt-users", kind="dev")]),
            package("systemprompt-users", "domain"),
        ])

    def test_repository_passes(self):
        result = subprocess.run(["bash", str(SCRIPT)], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
