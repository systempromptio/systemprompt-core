#!/usr/bin/env python3
"""lint-capability-deps refuses exactly the edges between two capabilities.

Each test writes a cargo metadata document and a unit table for a fixture
graph and runs the gate over them with `--metadata` and `--units`.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-capability-deps.py"

UNITS = """\
systemprompt-extension   kernel
systemprompt-runtime     kernel
systemprompt-content     capability:content
systemprompt-content-entry capability:content
systemprompt-files       capability:files
systemprompt-files-entry capability:files
"""


def package(name, deps=(), kind=None, optional=False):
    return {
        "name": name,
        "manifest_path": f"/fixture/crates/x/{name}/Cargo.toml",
        "dependencies": [{"name": d, "kind": kind, "optional": optional, "features": []} for d in deps],
    }


class CapabilityDeps(unittest.TestCase):
    def run_graph(self, packages):
        directory = tempfile.mkdtemp()
        self.addCleanup(lambda: [os.unlink(p) for p in Path(directory).iterdir()] and None)
        metadata = Path(directory) / "metadata.json"
        units = Path(directory) / "units.txt"
        metadata.write_text(json.dumps({"packages": packages}))
        units.write_text(UNITS)
        return subprocess.run(
            ["python3", str(SCRIPT), "--metadata", str(metadata), "--units", str(units)],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def check(self, packages, expected):
        base = [package(n) for n in ("systemprompt-extension", "systemprompt-runtime", "systemprompt-files")]
        names = {p["name"] for p in packages}
        result = self.run_graph([p for p in base if p["name"] not in names] + packages)
        lines = sorted(line.split(":")[0].strip() for line in result.stdout.splitlines() if line.startswith("  "))
        self.assertEqual(lines, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_a_capability_pair_and_kernel_edges_pass(self):
        self.check([
            package("systemprompt-content", ["systemprompt-extension"]),
            package("systemprompt-content-entry", ["systemprompt-content", "systemprompt-extension"]),
            package("systemprompt-runtime", ["systemprompt-extension"]),
        ], [])

    def test_capability_on_another_capability_fails(self):
        self.check([package("systemprompt-content", ["systemprompt-files"])],
                   ["systemprompt-content -> systemprompt-files"])

    def test_entry_on_another_capability_fails(self):
        self.check([package("systemprompt-content-entry", ["systemprompt-files-entry"]),
                    package("systemprompt-files-entry")],
                   ["systemprompt-content-entry -> systemprompt-files-entry"])

    def test_optional_edge_counts(self):
        self.check([package("systemprompt-content", ["systemprompt-files"], optional=True)],
                   ["systemprompt-content -> systemprompt-files"])

    def test_dev_edge_does_not_count(self):
        self.check([package("systemprompt-content", ["systemprompt-files"], kind="dev")], [])

    def test_kernel_edges_are_not_judged(self):
        self.check([package("systemprompt-runtime", ["systemprompt-files"])], [])


if __name__ == "__main__":
    unittest.main()
