#!/usr/bin/env python3
"""The gate scope table covers check-gates.txt exactly, and every unit gate
honours the shared `--paths` scope.

Checks: every gate in scripts/check-gates.txt has exactly one row in
scripts/gate-scopes.txt and vice versa; scope and cost come from the allowed
vocabulary; each unit gate's script reaches the shared helper; and the helper
narrows roots, filters lines and enumerates untracked files as documented.
"""
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
sys.dont_write_bytecode = True
sys.path.insert(0, str(SCRIPTS / "lib"))
import gate_scope  # noqa: E402

SCOPES = {"unit", "global"}
COSTS = {"cheap", "compile", "db"}
HELPER_MARKERS = ("lib/gate-scope.sh", "lib/rust-contracts.sh", "import gate_scope")


def gates():
    return gate_scope.gate_list()


def rows():
    out = []
    for line in gate_scope.GATE_TABLE.read_text().splitlines():
        if line.strip() and not line.startswith("#"):
            out.append(line.split(None, 3))
    return out


def recipe_script(gate):
    text = (ROOT / "justfile").read_text()
    m = re.search(rf"^{re.escape(gate)}(?: \*ARGS)?:\n((?:    .*\n)+)", text, re.M)
    if m is None:
        return None
    found = re.search(r"scripts/[\w.-]+\.(?:sh|py)", m.group(1))
    return ROOT / found.group(0) if found else None


class GateScopes(unittest.TestCase):
    def test_table_matches_check_gates(self):
        listed = [r[0] for r in rows()]
        self.assertEqual(sorted(listed), sorted(set(listed)), "duplicate rows")
        self.assertEqual(sorted(listed), sorted(gates()))

    def test_vocabulary(self):
        for row in rows():
            self.assertGreaterEqual(len(row), 3, row)
            self.assertIn(row[1], SCOPES, row)
            self.assertIn(row[2], COSTS, row)

    def test_unit_gates_source_the_helper(self):
        for name, scope, *_ in rows():
            if scope != "unit":
                continue
            script = recipe_script(name)
            self.assertIsNotNone(script, f"{name}: no script in its recipe")
            self.assertTrue(
                any(m in script.read_text() for m in HELPER_MARKERS),
                f"{name}: {script.name} does not use scripts/lib/gate-scope",
            )
            self.assertRegex(
                (ROOT / "justfile").read_text(),
                rf"(?m)^{re.escape(name)} \*ARGS:",
                f"{name}: recipe does not forward arguments",
            )

    def test_parse(self):
        self.assertEqual(gate_scope.parse(["--count"]), (["--count"], None))
        self.assertEqual(
            gate_scope.parse(["--count", "--paths", "./crates/x/", str(ROOT / "bin")]),
            (["--count"], ["crates/x", "bin"]),
        )
        with self.assertRaises(gate_scope.ScopeError):
            gate_scope.parse(["--paths"])

    def test_roots_and_filter(self):
        self.assertEqual(
            gate_scope.roots(["crates/shared", "crates/domain"], ["crates/domain/files"]),
            ["crates/domain/files"],
        )
        self.assertEqual(gate_scope.roots(["crates/shared"], None), ["crates/shared"])
        self.assertTrue(gate_scope.in_scope("crates/domain/files/src/lib.rs:3:x".split(":")[0], ["crates/domain/files"]))
        self.assertFalse(gate_scope.in_scope("crates/domain/filesx/a.rs", ["crates/domain/files"]))

    def test_untracked_file_is_enumerated_and_scoped(self):
        with tempfile.NamedTemporaryFile(dir=ROOT / "crates", suffix=".rs") as probe:
            rel = Path(probe.name).relative_to(ROOT).as_posix()
            self.assertIn(rel, gate_scope.files(["crates/*.rs"]))
            self.assertNotIn(rel, gate_scope.files(["crates/*.rs"], ["crates/domain"]))

    def test_scoped_gate_skips_out_of_scope_violation(self):
        probe = ROOT / "crates" / "domain" / "files" / "src" / "zz_gate_scope_probe.rs"
        probe.write_text("pub fn go() {\n    tokio::spawn(async {});\n}\n")
        self.addCleanup(probe.unlink)
        run = lambda *a: subprocess.run(
            ["bash", str(SCRIPTS / "lint-owned-tasks.sh"), *a],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )
        self.assertNotEqual(run("--paths", "crates/domain/files").returncode, 0)
        self.assertEqual(run("--paths", "crates/domain/ai").returncode, 0)


if __name__ == "__main__":
    unittest.main()
