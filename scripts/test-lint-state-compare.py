#!/usr/bin/env python3
"""lint-state-compare flags a state field compared against a string literal.

Each test writes one fixture source file into the repository (untracked files
are in a gate's scope), runs the gate scoped to it with `--paths`, and removes
it again. A comparison outside the exempt trees fails; the same text under
crates/shared/wire/ or crates/tests/ and an enum comparison pass.
"""
import os
from pathlib import Path
import subprocess
import unittest
import uuid

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-state-compare.sh"


class StateCompare(unittest.TestCase):
    def setUp(self):
        self.created = []

    def tearDown(self):
        for path in self.created:
            path.unlink(missing_ok=True)

    def fixture(self, directory, body):
        path = ROOT / directory / f"lint_state_compare_{uuid.uuid4().hex}.rs"
        path.write_text(body, encoding="utf-8")
        self.created.append(path)
        return path.relative_to(ROOT).as_posix()

    def run_gate(self, rel):
        return subprocess.run(
            ["bash", str(SCRIPT), "--paths", rel],
            cwd=ROOT,
            capture_output=True,
            text=True,
            env=os.environ | {"LC_ALL": "C"},
        )

    def test_string_comparison_fails(self):
        for line in ('row.status == "failed"', 'r.kind.as_str() == "skill"',
                     'msg.role == "system"', 'part.state == "error"'):
            with self.subTest(line=line):
                rel = self.fixture("crates/entry/cli/src", f"fn f() -> bool {{ {line} }}\n")
                result = self.run_gate(rel)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn(rel, result.stderr)

    def test_enum_comparison_passes(self):
        rel = self.fixture("crates/entry/cli/src", "fn f() -> bool { row.status == RunStatus::Failed }\n")
        result = self.run_gate(rel)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_wire_and_tests_are_exempt(self):
        for directory in ("crates/shared/wire/src", "crates/tests/unit"):
            with self.subTest(directory=directory):
                rel = self.fixture(directory, 'fn f() -> bool { role == "user" }\n')
                result = self.run_gate(rel)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_core_tree_passes(self):
        result = subprocess.run(["bash", str(SCRIPT)], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
