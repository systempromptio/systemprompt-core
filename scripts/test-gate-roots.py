#!/usr/bin/env python3
"""Every root-walking gate scans the roots it is given and defaults to core.

Each test writes a source tree with one violation into a temporary directory,
runs the gate against that root (expecting the violation to be reported), runs
it with an allowlist naming the fixture where the gate has one (expecting a
pass), and runs the gate with no arguments (expecting the core tree to pass and
the fixture to stay out of the report).
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"


def run(script, *args, cwd=ROOT):
    return subprocess.run(
        ["bash", str(SCRIPTS / script), *args],
        cwd=cwd,
        capture_output=True,
        text=True,
        env=os.environ | {"LC_ALL": "C"},
    )


class GateRoots(unittest.TestCase):
    def fixture(self, relative, body):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name) / "fixture"
        path = root / relative
        path.parent.mkdir(parents=True)
        path.write_text(body)
        return root, path

    def allowlist(self, body):
        self.allowlists = getattr(self, "allowlists", 0) + 1
        path = Path(self.tmp.name) / f"allowlist-{self.allowlists}.txt"
        path.write_text(body)
        return path

    def assert_reports(self, result, path):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(path.name, result.stdout + result.stderr)

    def assert_default_is_core(self, script, path):
        result = run(script)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn(path.name, result.stdout + result.stderr)

    def test_owned_tasks(self):
        root, path = self.fixture(
            "src/spawner.rs", "pub fn go() {\n    tokio::spawn(async {});\n}\n"
        )
        self.assert_reports(run("lint-owned-tasks.sh", str(root)), path)
        owners = self.allowlist(f"{path}\n")
        allowed = run("lint-owned-tasks.sh", "--allowlist", str(owners), str(root))
        self.assertEqual(allowed.returncode, 0, allowed.stdout + allowed.stderr)
        self.assert_default_is_core("lint-owned-tasks.sh", path)

    def test_async_trait(self):
        root, path = self.fixture(
            "src/unused_dyn.rs",
            "#[async_trait::async_trait]\npub trait Fetch {\n    async fn fetch(&self);\n}\n",
        )
        self.assert_reports(run("lint-async-trait.sh", str(root)), path)
        self.assert_default_is_core("lint-async-trait.sh", path)

    def test_stringly_errors(self):
        root, path = self.fixture(
            "src/stringly.rs", "pub fn parse() -> Result<(), String> {\n    Ok(())\n}\n"
        )
        self.assert_reports(run("lint-stringly-errors.sh", str(root)), path)
        self.assert_default_is_core("lint-stringly-errors.sh", path)

    def test_migrations(self):
        root, path = self.fixture(
            "crates/domain/users/schema/migrations/099_unsafe.sql",
            "ALTER TABLE user_sessions ADD CONSTRAINT user_sessions_kind_check CHECK (kind IN ('a'));\n"
            "CREATE INDEX idx_user_sessions_kind ON user_sessions(kind);\n"
            "UPDATE user_sessions SET kind = 'a';\n"
            "ALTER TABLE user_sessions ADD COLUMN kind TEXT NOT NULL;\n"
            "CREATE INDEX CONCURRENTLY idx_user_sessions_kind2 ON user_sessions(kind);\n"
            "CREATE OR REPLACE FUNCTION is_kind(kind TEXT) RETURNS BOOLEAN LANGUAGE plpgsql AS $$ BEGIN RETURN true; END $$;\n"
            "CREATE OR REPLACE VIEW v_kind AS SELECT * FROM user_sessions WHERE is_kind(kind);\n",
        )
        result = run("lint-migrations.sh", str(root))
        self.assert_reports(result, path)
        for rule in ("constraint", "index", "bulk-write", "not-null", "inline"):
            self.assertIn(f"[{rule}]", result.stdout, result.stdout + result.stderr)
        safe = Path(self.tmp.name) / "safe" / "crates" / "domain" / "users" / "schema" / "migrations" / "099_safe.sql"
        safe.parent.mkdir(parents=True)
        safe.write_text(
            "-- @no-transaction\n"
            "ALTER TABLE user_sessions ADD CONSTRAINT user_sessions_kind_check CHECK (kind IN ('a')) NOT VALID;\n"
            "ALTER TABLE user_sessions VALIDATE CONSTRAINT user_sessions_kind_check;\n"
            "CREATE INDEX CONCURRENTLY idx_user_sessions_kind ON user_sessions(kind);\n"
            "-- Why: bounded: one row per fixture.\n"
            "UPDATE user_sessions SET kind = 'a';\n"
            "DELETE FROM user_sessions WHERE ctid = ANY(ARRAY(SELECT ctid FROM user_sessions LIMIT 10));\n"
            "ALTER TABLE user_sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'a';\n"
        )
        passed = run("lint-migrations.sh", str(safe.parents[5]))
        self.assertEqual(passed.returncode, 0, passed.stdout + passed.stderr)
        self.assert_default_is_core("lint-migrations.sh", path)

    def test_env_vars(self):
        root, path = self.fixture(
            "src/reader.rs", 'pub fn home() -> Option<String> {\n    std::env::var("HOME").ok()\n}\n'
        )
        empty = self.allowlist("")
        self.assert_reports(run("lint-env-vars.sh", "--allowlist", str(empty), str(root)), path)
        listed = self.allowlist(f"{path}  # fixture: HOME probe\n")
        allowed = run("lint-env-vars.sh", "--allowlist", str(listed), str(root))
        self.assertEqual(allowed.returncode, 0, allowed.stdout + allowed.stderr)
        self.assert_default_is_core("lint-env-vars.sh", path)

    def test_repo_construction(self):
        root, path = self.fixture(
            "src/handlers.rs",
            "pub fn handle(pool: &Pool) {\n    let _repo = UserRepository::new(pool);\n}\n",
        )
        empty = self.allowlist("")
        self.assert_reports(run("lint-repo-construction.sh", "--allowlist", str(empty), str(root)), path)
        listed = self.allowlist(f"{path}\n")
        allowed = run("lint-repo-construction.sh", "--allowlist", str(listed), str(root))
        self.assertEqual(allowed.returncode, 0, allowed.stdout + allowed.stderr)
        self.assert_default_is_core("lint-repo-construction.sh", path)

    def test_sqlx(self):
        root, path = self.fixture(
            "src/raw.rs", 'pub fn q() {\n    let _ = sqlx::query("SELECT 1");\n}\n'
        )
        empty = self.allowlist("")
        self.assert_reports(run("check-sqlx.sh", "--allowlist", str(empty), str(root)), path)
        listed = self.allowlist(f"{path}\n")
        allowed = run("check-sqlx.sh", "--allowlist", str(listed), str(root))
        self.assertEqual(allowed.returncode, 0, allowed.stdout + allowed.stderr)
        self.assert_default_is_core("check-sqlx.sh", path)

    def test_inline_tests(self):
        root, path = self.fixture(
            "src/with_tests.rs", "pub fn f() {}\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n"
        )
        self.assert_reports(run("lint-inline-tests.sh", str(root)), path)
        self.assert_default_is_core("lint-inline-tests.sh", path)

    def test_test_seams(self):
        root, path = self.fixture(
            "src/seam.rs", '#[cfg(feature = "test-seam")]\npub fn only_in_tests() {}\n'
        )
        self.assert_reports(run("lint-test-seams.sh", str(root)), path)
        self.assert_default_is_core("lint-test-seams.sh", path)

    def test_tracing_messages(self):
        root, path = self.fixture(
            "src/log.rs",
            'pub fn log(name: &str) {\n    tracing::info!("started {}", name);\n}\n',
        )
        self.assert_reports(run("lint-tracing-messages.sh", str(root)), path)
        self.assert_default_is_core("lint-tracing-messages.sh", path)

    def test_core_globals(self):
        root, path = self.fixture(
            "src/state.rs",
            "use std::sync::OnceLock;\nstatic CACHE: OnceLock<String> = OnceLock::new();\n",
        )
        empty = self.allowlist("")
        self.assert_reports(run("lint-core-globals.sh", "--allowlist", str(empty), str(root)), path)
        listed = self.allowlist(f"{path}:CACHE — fixture: immutable data\n")
        allowed = run("lint-core-globals.sh", "--allowlist", str(listed), str(root))
        self.assertEqual(allowed.returncode, 0, allowed.stdout + allowed.stderr)
        self.assert_default_is_core("lint-core-globals.sh", path)

    def test_serde_aliases(self):
        root, path = self.fixture(
            "src/wire.rs",
            '#[derive(serde::Deserialize)]\npub struct Row {\n    #[serde(alias = "old")]\n    pub new: String,\n}\n',
        )
        empty = self.allowlist("")
        self.assert_reports(run("lint-serde-aliases.sh", "--allowlist", str(empty), str(root)), path)
        listed = self.allowlist(f"{path}:new — fixture: vendor spelling\n")
        allowed = run("lint-serde-aliases.sh", "--allowlist", str(listed), str(root))
        self.assertEqual(allowed.returncode, 0, allowed.stdout + allowed.stderr)
        ports, port_path = self.fixture("src/ports.rs", "pub const PORT: u16 = 8080;\n")
        no_aliases = self.allowlist("")
        self.assert_reports(
            run("lint-serde-aliases.sh", "--allowlist", str(no_aliases), str(ports)), port_path
        )
        self.assert_default_is_core("lint-serde-aliases.sh", path)


if __name__ == "__main__":
    unittest.main()
