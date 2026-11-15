#!/usr/bin/env python3
"""lint-kernel-deps refuses exactly the kernel→capability and entry→kernel edges.

Each test writes a cargo metadata document for a fixture graph and runs the
gate over it with `--metadata`.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-kernel-deps.py"


def package(name, layer, deps=(), kind=None):
    return {
        "name": name,
        "manifest_path": f"/fixture/crates/{layer}/{name}/Cargo.toml",
        "dependencies": [{"name": d, "kind": kind, "optional": False, "features": []} for d in deps],
    }


BASE = [
    package("systemprompt-extension", "shared"),
    package("systemprompt-users", "domain"),
    package("systemprompt-analytics-entry", "entry", ["systemprompt-extension"]),
]


class KernelDeps(unittest.TestCase):
    def run_graph(self, packages):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as out:
            json.dump({"packages": packages}, out)
        self.addCleanup(os.unlink, out.name)
        return subprocess.run(
            ["python3", str(SCRIPT), "--metadata", out.name],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def check(self, packages, expected):
        result = self.run_graph(BASE + packages)
        lines = sorted(line.split(":")[0].strip() for line in result.stdout.splitlines() if line.startswith("  "))
        self.assertEqual(lines, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_clean_graph_passes(self):
        self.check([package("systemprompt-api", "entry", ["systemprompt-extension"])], [])

    def test_kernel_on_domain_fails(self):
        self.check(
            [package("systemprompt-runtime", "app", ["systemprompt-users"])],
            ["systemprompt-runtime -> systemprompt-users"],
        )

    def test_every_kernel_crate_is_judged(self):
        kernels = ["systemprompt-runtime", "systemprompt-api", "systemprompt-cli", "systemprompt-scheduler", "systemprompt-mcp-host"]
        self.check(
            [package(k, "app", ["systemprompt-analytics-entry"]) for k in kernels],
            [f"{k} -> systemprompt-analytics-entry" for k in kernels],
        )

    def test_entry_on_cli_or_api_fails(self):
        self.check(
            [
                package("systemprompt-cli", "entry"),
                package("systemprompt-api", "entry"),
                package("systemprompt-content-entry", "entry", ["systemprompt-cli", "systemprompt-api"]),
            ],
            ["systemprompt-content-entry -> systemprompt-api", "systemprompt-content-entry -> systemprompt-cli"],
        )

    def test_non_kernel_app_crate_may_use_domain(self):
        self.check([package("systemprompt-gateway", "app", ["systemprompt-users"])], [])

    def test_dev_dependency_is_ignored(self):
        self.check([package("systemprompt-runtime", "app", ["systemprompt-users"], kind="dev")], [])


if __name__ == "__main__":
    unittest.main()
