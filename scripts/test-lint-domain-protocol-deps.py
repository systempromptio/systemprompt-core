#!/usr/bin/env python3
"""lint-domain-protocol-deps refuses a domain crate's unconditional HTTP/CLI stack.

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
SCRIPT = ROOT / "scripts" / "lint-domain-protocol-deps.py"


def package(name, layer, deps=(), features=None, kind=None):
    return {
        "name": name,
        "manifest_path": f"/fixture/crates/{layer}/{name}/Cargo.toml",
        "features": features or {},
        "dependencies": [{"name": d, "kind": kind, "optional": optional, "features": []} for d, optional in deps],
    }


class DomainProtocolDeps(unittest.TestCase):
    def check(self, packages, expected):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as out:
            json.dump({"packages": packages}, out)
        self.addCleanup(os.unlink, out.name)
        result = subprocess.run(["python3", str(SCRIPT), "--metadata", out.name],
                                cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"})
        lines = sorted(line.split(":")[0].strip() for line in result.stdout.splitlines() if line.startswith("  "))
        self.assertEqual(lines, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_unconditional_axum_in_a_domain_crate_fails(self):
        self.check([package("systemprompt-agent", "domain", [("axum", False)])], ["systemprompt-agent -> axum"])

    def test_every_protocol_crate_is_judged(self):
        self.check([package("systemprompt-agent", "domain", [(d, False) for d in ("axum", "clap", "tower", "tower-http")])],
                   [f"systemprompt-agent -> {d}" for d in ("axum", "clap", "tower", "tower-http")])

    def test_optional_behind_http_or_cli_passes(self):
        self.check([package("systemprompt-marketplace", "domain", [("axum", True), ("clap", True)],
                            {"http": ["dep:axum"], "cli": ["dep:clap"]})], [])

    def test_optional_behind_another_feature_fails(self):
        self.check([package("systemprompt-marketplace", "domain", [("axum", True)], {"default": ["dep:axum"]})],
                   ["systemprompt-marketplace -> axum"])

    def test_entry_crates_are_not_judged(self):
        self.check([package("systemprompt-agent-entry", "entry", [("axum", False)])], [])

    def test_dev_dependency_does_not_count(self):
        self.check([package("systemprompt-agent", "domain", [("axum", False)], kind="dev")], [])


if __name__ == "__main__":
    unittest.main()
