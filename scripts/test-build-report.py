#!/usr/bin/env python3
"""The build report's crate closure comes from one cargo metadata resolve.

`scripts/build-report.sh` counts a composition's closure with
`scripts/lib/cargo_graph.py closure` over a host-filtered `cargo metadata`;
these tests pin the walk: normal and build edges count, dev edges do not, the
root is excluded, path packages are counted separately, and the count depends
on nothing but the resolve.
"""
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts" / "lib"))
import cargo_graph  # noqa: E402


def dep(pkg, *kinds):
    return {"pkg": pkg, "dep_kinds": [{"kind": k, "target": None} for k in kinds]}


METADATA = {
    "packages": [
        {"id": "root", "source": None},
        {"id": "facade", "source": None},
        {"id": "serde", "source": "registry+https://github.com/rust-lang/crates.io-index"},
        {"id": "cc", "source": "registry+https://github.com/rust-lang/crates.io-index"},
        {"id": "proptest", "source": "registry+https://github.com/rust-lang/crates.io-index"},
        {"id": "shared", "source": "registry+https://github.com/rust-lang/crates.io-index"},
    ],
    "resolve": {
        "root": "root",
        "nodes": [
            {"id": "root", "deps": [dep("facade", None)]},
            {"id": "facade", "deps": [dep("serde", None), dep("cc", "build"), dep("proptest", "dev")]},
            {"id": "serde", "deps": [dep("shared", None)]},
            {"id": "cc", "deps": [dep("shared", None)]},
            {"id": "proptest", "deps": []},
            {"id": "shared", "deps": []},
        ],
    },
}


class Closure(unittest.TestCase):
    def test_normal_and_build_edges_without_dev_or_root(self):
        self.assertEqual(cargo_graph.resolved_closure(METADATA), {"facade", "serde", "cc", "shared"})

    def test_path_packages(self):
        closure = cargo_graph.resolved_closure(METADATA)
        self.assertEqual(cargo_graph.path_packages(METADATA, closure), {"facade"})

    def test_edge_listed_with_dev_and_normal_counts(self):
        metadata = json.loads(json.dumps(METADATA))
        metadata["resolve"]["nodes"][1]["deps"][2] = dep("proptest", "dev", None)
        self.assertIn("proptest", cargo_graph.resolved_closure(metadata))

    def test_no_resolve_root_is_an_error(self):
        with self.assertRaises(ValueError):
            cargo_graph.resolved_closure({"packages": [], "resolve": None})

    def test_command_prints_packages_and_path_packages(self):
        result = subprocess.run([sys.executable, "-B", str(ROOT / "scripts" / "lib" / "cargo_graph.py"), "closure"],
                                input=json.dumps(METADATA), capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.split(), ["4", "1"])

    def test_report_reads_the_closure_from_the_resolve(self):
        text = (ROOT / "scripts" / "build-report.sh").read_text()
        self.assertIn("cargo_graph.py\" closure", text)
        self.assertIn("--filter-platform", text)
        self.assertNotIn("cargo tree", text)


if __name__ == "__main__":
    unittest.main()
