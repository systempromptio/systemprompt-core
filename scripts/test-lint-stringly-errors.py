#!/usr/bin/env python3
"""lint-stringly-errors reports a bare String error variant in the strict layers.

Each test writes a crate tree into a temporary directory and runs the gate on
its `crates/<layer>` root: an error enum's `Foo(String)` variant is reported
in domain, app and infra whatever builds it; a non-error enum with the same
shape and the same variant in a shared crate are not.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
GATE = ROOT / "scripts" / "lint-stringly-errors.sh"

ERROR_ENUM = """#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("bad {0}")]
    Bad(String),
    #[error("typed {name}")]
    Typed { name: String },
}
"""

DATA_ENUM = """#[derive(Debug, thiserror::Error)]
#[error("x")]
pub struct Unrelated;

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum Shape {
    Text(String),
}
"""


def scan(tree, layer):
    with tempfile.TemporaryDirectory() as tmp:
        for relative, body in tree.items():
            path = Path(tmp) / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(body, encoding="utf-8")
        return subprocess.run(
            ["bash", str(GATE), f"crates/{layer}"],
            cwd=tmp,
            capture_output=True,
            text=True,
            env=os.environ | {"LC_ALL": "C"},
        )


class LayerStringVariant(unittest.TestCase):
    def test_an_error_variant_in_each_strict_layer_is_reported(self):
        for layer in ("domain", "app", "infra"):
            with self.subTest(layer=layer):
                result = scan({f"crates/{layer}/fixture/src/error.rs": ERROR_ENUM}, layer)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn("layer-string-variant", result.stdout)
                self.assertIn("error.rs:4", result.stdout)
                self.assertNotIn("error.rs:6", result.stdout)

    def test_a_data_enum_beside_an_error_type_is_not_reported(self):
        result = scan({"crates/domain/fixture/src/shape.rs": DATA_ENUM}, "domain")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_shared_crate_variant_is_left_to_the_display_rule(self):
        result = scan({"crates/shared/fixture/src/error.rs": ERROR_ENUM}, "shared")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
