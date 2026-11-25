#!/usr/bin/env python3
"""lint-capability-cfg refuses a capability cfg anywhere but on a module.

Each test builds a fixture repository (unit table, facade manifest, one
source file) and runs the gate over it with `--root`.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-capability-cfg.py"

UNITS = "systemprompt-runtime kernel\nsystemprompt-brain capability:brain\nsystemprompt-otel capability:telemetry\n"
FACADE = """\
[features]
api = ["dep:systemprompt-brain"]
otel = ["systemprompt-otel?/export"]
logging = ["dep:systemprompt-logging"]

[dependencies]
"""


class CapabilityCfg(unittest.TestCase):
    def check(self, source, expected, path="crates/app/runtime/src/lib.rs"):
        root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, root)
        (root / "scripts").mkdir()
        (root / "scripts" / "units.txt").write_text(UNITS)
        (root / "systemprompt").mkdir()
        (root / "systemprompt" / "Cargo.toml").write_text(FACADE)
        target = root / path
        target.parent.mkdir(parents=True)
        target.write_text(source)
        result = subprocess.run(["python3", str(SCRIPT), "--root", str(root)],
                                cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"})
        lines = [line.split(":")[1] for line in result.stdout.splitlines() if line.startswith("  ")]
        self.assertEqual(lines, expected, result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_cfg_on_a_mod_or_its_reexport_passes(self):
        self.check('#[cfg(feature = "brain")]\nmod brain;\n#[cfg(feature = "brain")]\npub use brain::Brain;\n', [])

    def test_cfg_on_a_fn_fails(self):
        self.check('#[cfg(feature = "brain")]\nfn recall() {}\n', ["1"])

    def test_cfg_on_a_statement_fails(self):
        self.check('fn boot() {\n    #[cfg(feature = "otel")]\n    start();\n}\n', ["2"])

    def test_feature_enabling_a_capability_package_counts(self):
        self.check('#[cfg(feature = "otel")]\nconst X: u8 = 1;\n', ["1"])

    def test_surface_and_infra_features_are_not_capabilities(self):
        self.check('#[cfg(feature = "api")]\nfn a() {}\n#[cfg(feature = "logging")]\nfn b() {}\n', [])

    def test_attribute_stack_is_skipped_to_the_item(self):
        self.check('#[cfg(feature = "brain")]\n#[cfg_attr(docsrs, doc(cfg(feature = "brain")))]\npub mod brain;\n', [])

    def test_the_facade_is_a_composition_root(self):
        self.check('#[cfg(feature = "brain")]\nfn compose() {}\n', [], path="systemprompt/src/lib.rs")

    def test_test_workspace_is_not_scanned(self):
        self.check('#[cfg(feature = "brain")]\nfn t() {}\n', [], path="crates/tests/unit/x/src/lib.rs")


if __name__ == "__main__":
    unittest.main()
