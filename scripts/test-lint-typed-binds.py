#!/usr/bin/env python3
"""lint-typed-binds reports exactly the converted bind arguments it bans.

Each test writes a repository tree into a temporary directory and runs the
gate with that directory as its ROOT.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-typed-binds.py"
REPO = "crates/domain/users/src/repository/user.rs"


class TypedBinds(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        (self.base / "crates").mkdir()

    def write(self, rel, text):
        path = self.base / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def run_gate(self):
        return subprocess.run(
            ["python3", str(SCRIPT), str(self.base)],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def check(self, expected):
        result = self.run_gate()
        found = sorted(
            " ".join(line.strip().split(" ", 2)[:2]).replace(": ", " ").rstrip(":")
            for line in result.stdout.splitlines() if line.startswith("  ")
        )
        self.assertEqual(found, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_identifier_as_str_is_reported(self):
        self.write(REPO, 'fn f(user_id: &UserId) {\n    sqlx::query!("SELECT 1 WHERE id = $1", user_id.as_str());\n}\n')
        self.check([f"{REPO}:2 [identifier]"])

    def test_bound_as_itself_passes(self):
        self.write(REPO, 'fn f(user_id: &UserId) {\n    sqlx::query!("SELECT 1 WHERE id = $1", user_id as _);\n}\n')
        self.check([])

    def test_conversion_text_inside_the_sql_is_not_an_argument(self):
        self.write(REPO, 'fn f() {\n    sqlx::query!(r#"SELECT \'x.as_str()\' AS "a: UserId""#);\n}\n')
        self.check([])

    def test_row_type_and_path_function_form(self):
        self.write(
            REPO,
            "fn f(owner: Option<&UserId>) {\n"
            '    sqlx::query_as!(Row, "SELECT 1 WHERE o = $1", owner.map(UserId::as_str));\n'
            "}\n",
        )
        self.check([f"{REPO}:2 [identifier]"])

    def test_variant_and_to_string_are_counted_not_judged(self):
        self.write(
            REPO,
            "fn f(n: i64) {\n"
            "    query_scalar!(\n"
            '        "SELECT 1 WHERE s = $1 AND n = $2",\n'
            "        UserStatus::Active.as_str(),\n"
            "        n.to_string()\n"
            "    );\n"
            "}\n",
        )
        self.check([])

    def test_comments_and_other_macros_are_ignored(self):
        self.write(
            REPO,
            "fn f(id: &UserId) {\n"
            '    my_query!("x", id.as_str());\n'
            '    other::query!("x", id.as_str());\n'
            '    sqlx::query!("SELECT $1", id as _ /* id.as_str() */);\n'
            "}\n",
        )
        self.check([])

    def test_test_workspace_is_not_scanned(self):
        self.write(
            "crates/tests/unit/src/lib.rs",
            'fn f(id: &UserId) { sqlx::query!("SELECT $1", id.as_str()); }\n',
        )
        self.check([])


if __name__ == "__main__":
    unittest.main()
