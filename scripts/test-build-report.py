#!/usr/bin/env python3
"""The build report's crate closure is the feature-resolved `cargo tree`.

`scripts/build-report.sh` counts a composition's closure with
`scripts/lib/cargo_graph.py tree-closure` over `cargo tree -e normal,build
--prefix none --target <host>`; these tests pin the parse: the root line is
excluded, duplicates (`(*)`) count once, path packages are counted apart from
registry ones, and only capability-unit path packages are reported.
"""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts" / "lib"))
import cargo_graph  # noqa: E402

TREE = """build-report-runtime v0.0.0 (/w/target/build-report/runtime)
systemprompt v0.87.0 (/w/systemprompt)
systemprompt-runtime v0.87.0 (/w/crates/app/runtime)
serde v1.0.228
serde_derive v1.0.228 (proc-macro)
systemprompt-runtime v0.87.0 (/w/crates/app/runtime) (*)
systemprompt-macros v0.87.0 (proc-macro) (/w/crates/shared/macros)
cc v1.2.0
"""


class TreeClosure(unittest.TestCase):
    def test_root_excluded_and_duplicates_once(self):
        closure = cargo_graph.tree_closure(TREE)
        self.assertEqual(len(closure), 6)
        self.assertNotIn(("build-report-runtime", "0.0.0"), closure)

    def test_path_packages_are_the_ones_with_a_path(self):
        closure = cargo_graph.tree_closure(TREE)
        paths = sorted(name for (name, _), path in closure.items() if path is not None)
        self.assertEqual(paths, ["systemprompt", "systemprompt-macros", "systemprompt-runtime"])

    def test_capabilities_are_capability_path_packages(self):
        units = {"systemprompt-runtime": "kernel", "systemprompt-macros": "capability:x", "serde": "capability:y"}
        self.assertEqual(cargo_graph.tree_capabilities(cargo_graph.tree_closure(TREE), units),
                         ["systemprompt-macros"])

    def test_a_facade_feature_that_is_off_is_not_in_the_closure(self):
        units = {"systemprompt-gateway": "capability:gateway"}
        self.assertEqual(cargo_graph.tree_capabilities(cargo_graph.tree_closure(TREE), units), [])

    def test_command_prints_packages_and_path_packages(self):
        result = subprocess.run([sys.executable, "-B", str(ROOT / "scripts" / "lib" / "cargo_graph.py"), "tree-closure"],
                                input=TREE, capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.split(), ["6", "3"])

    def test_report_reads_the_closure_from_cargo_tree(self):
        text = (ROOT / "scripts" / "build-report.sh").read_text()
        self.assertIn("cargo_graph.py\" tree-closure", text)
        self.assertIn("cargo_graph.py\" tree-capabilities", text)
        self.assertIn("-e normal,build --prefix none --target", text)
        self.assertIn("capability_packages", text)



class EmptyInput(unittest.TestCase):
    def test_merge_with_no_sidecar_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            result = subprocess.run(["bash", str(ROOT / "scripts" / "build-report.sh"), "--merge", tmp,
                                     str(Path(tmp) / "report.md")],
                                    capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("found no composition sidecar", result.stderr)

    def test_an_empty_closure_fails_the_report(self):
        text = (ROOT / "scripts" / "build-report.sh").read_text()
        self.assertIn('[ "${packages:-0}" -gt 0 ]', text)

    def test_tree_closure_of_nothing_is_zero_packages(self):
        result = subprocess.run([sys.executable, "-B", str(ROOT / "scripts" / "lib" / "cargo_graph.py"), "tree-closure"],
                                input="", capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.split()[0], "0")

    def test_the_floor_check_fails_an_empty_floor_closure(self):
        text = (ROOT / "scripts" / "compositions.sh").read_text()
        self.assertIn('select(.packages == 0)', text)
        self.assertIn('empty floor closure', text)


if __name__ == "__main__":
    unittest.main()
