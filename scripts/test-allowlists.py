#!/usr/bin/env python3
"""Per-unit allowlists: one reader, every allowlist gate, the stale-entry rule.

The reader tests drive scripts/lib/gate_scope.py directly. The gate tests copy
the gate scripts into a throwaway git repository holding one package with one
violation, then run each gate in full mode three times: with no per-unit
allowlist (the violation is reported), with `<package>/gates/<gate>.allow`
naming it (the gate passes), and with a stale entry beside it (the gate fails
and names the stale entry).
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
sys.dont_write_bytecode = True
sys.path.insert(0, str(SCRIPTS / "lib"))
import gate_scope  # noqa: E402

MANIFEST = '[package]\nname = "demo"\nversion = "0.0.0"\n'


class Reader(unittest.TestCase):
    def test_allowlist_gates_are_check_gates(self):
        gates = set(gate_scope.gate_list())
        self.assertLessEqual(set(gate_scope.ALLOWLIST_GATES), gates)
        self.assertLessEqual(set(gate_scope.CENTRAL_ALLOWLISTS), set(gate_scope.ALLOWLIST_GATES))

    def test_rebase_keeps_the_rest_of_the_line(self):
        cases = {
            "src/a.rs  # reason": "crates/x/y/src/a.rs  # reason",
            "src/a.rs:FIELD — reason": "crates/x/y/src/a.rs:FIELD — reason",
            "src/repo/|table|reason": "crates/x/y/src/repo/|table|reason",
            "src/q.rs#0123abcd": "crates/x/y/src/q.rs#0123abcd",
            "templates/*.html": "crates/x/y/templates/*.html",
            "src/admin/": "crates/x/y/src/admin/",
        }
        for line, want in cases.items():
            with self.subTest(line=line):
                self.assertEqual(gate_scope.rebase(line, "crates/x/y", "f"), want)

    def test_rebase_refuses_paths_leaving_the_package(self):
        for line in ("../other/src/a.rs", "/etc/passwd", "src/../../z.rs"):
            with self.subTest(line=line), self.assertRaises(gate_scope.AllowlistError):
                gate_scope.rebase(line, "crates/x/y", "f")

    def test_owning_package_is_the_deepest(self):
        packages = ["crates/a", "crates/a/b"]
        self.assertEqual(gate_scope.owning_package("crates/a/b/src/x.rs", packages), "crates/a/b")
        self.assertEqual(gate_scope.owning_package("crates/a/src/x.rs", packages), "crates/a")
        self.assertIsNone(gate_scope.owning_package("crates/ab/x.rs", packages))

    def test_unknown_gate_is_refused(self):
        with self.assertRaises(gate_scope.AllowlistError):
            gate_scope.allowlist("lint-not-a-gate")

    def test_repository_layout_is_clean(self):
        self.assertEqual(gate_scope.check_allowlists(), [])


def git(cwd, *args):
    subprocess.run(["git", "-C", str(cwd), *args], check=True, capture_output=True,
                   env=os.environ | {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
                                     "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"})


class Fixture:
    """A git repository with the gate scripts and one package at `package`."""

    def __init__(self, tmp, package):
        self.root = Path(tmp) / "repo"
        self.package = package
        (self.root / "scripts").mkdir(parents=True)
        shutil.copytree(SCRIPTS / "lib", self.root / "scripts" / "lib",
                        ignore=shutil.ignore_patterns("__pycache__"))
        for name in ("check-gates.txt", "gate-scopes.txt"):
            shutil.copy(SCRIPTS / name, self.root / "scripts" / name)
        for gate, central in gate_scope.CENTRAL_ALLOWLISTS.items():
            (self.root / central).write_text(f"# central entries of {gate}\n")
        for placeholder in ("systemprompt/src/lib.rs", "bin/bridge/src/main.rs"):
            self.write(placeholder, "")
        self.write(f"{package}/Cargo.toml", MANIFEST)
        git(self.root, "init", "-q")

    def write(self, relative, body):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body)
        return path

    def allow(self, gate, *entries):
        path = self.root / self.package / "gates" / f"{gate}.allow"
        if entries:
            self.write(path.relative_to(self.root), "".join(f"{e}\n" for e in entries))
        elif path.exists():
            path.unlink()

    def run(self, script, *args):
        shutil.copy(SCRIPTS / script, self.root / "scripts" / script)
        git(self.root, "add", "-A")
        return subprocess.run(["bash", str(self.root / "scripts" / script), *args],
                              cwd=self.root, capture_output=True, text=True,
                              env=os.environ | {"LC_ALL": "C"})


class GatesReadPerUnitAllowlists(unittest.TestCase):
    def fixture(self, package="crates/domain/demo"):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        return Fixture(tmp.name, package)

    def check(self, fx, script, gate, good, stale, stale_mark):
        fx.allow(gate)
        bare = fx.run(script)
        self.assertNotEqual(bare.returncode, 0, bare.stdout + bare.stderr)
        fx.allow(gate, good)
        listed = fx.run(script)
        self.assertEqual(listed.returncode, 0, listed.stdout + listed.stderr)
        fx.allow(gate, good, stale)
        stale_run = fx.run(script)
        self.assertNotEqual(stale_run.returncode, 0, stale_run.stdout + stale_run.stderr)
        self.assertIn(stale_mark, stale_run.stdout + stale_run.stderr)

    def test_env_vars(self):
        fx = self.fixture()
        fx.write(f"{fx.package}/src/boot.rs", 'pub fn h() -> Option<String> {\n    std::env::var("HOME").ok()\n}\n')
        self.check(fx, "lint-env-vars.sh", "lint-env-vars", "src/boot.rs  # fixture: HOME probe",
                   "src/gone.rs  # fixture: gone", f"{fx.package}/src/gone.rs")

    def test_sqlx(self):
        fx = self.fixture()
        fx.write(f"{fx.package}/src/raw.rs", 'pub fn q() {\n    let _ = sqlx::query("SELECT 1");\n}\n')
        self.check(fx, "check-sqlx.sh", "lint-sqlx", "src/raw.rs", "src/gone.rs",
                   f"{fx.package}/src/gone.rs")

    def test_repo_construction(self):
        fx = self.fixture()
        fx.write(f"{fx.package}/src/http/handler.rs", "pub fn h(pool: &Pool) {\n    let _r = UserRepository::new(pool);\n}\n")
        self.check(fx, "lint-repo-construction.sh", "lint-repo-construction", "src/http/handler.rs",
                   "src/gone.rs", f"{fx.package}/src/gone.rs")

    def test_serde_aliases(self):
        fx = self.fixture()
        fx.write(f"{fx.package}/src/wire.rs",
                 '#[derive(serde::Deserialize)]\npub struct Row {\n    #[serde(alias = "old")]\n    pub new: String,\n}\n')
        self.check(fx, "lint-serde-aliases.sh", "lint-serde-aliases", "src/wire.rs:new — fixture: vendor",
                   "src/wire.rs:other — fixture: vendor", f"{fx.package}/src/wire.rs:other")

    def test_core_globals(self):
        fx = self.fixture()
        fx.write(f"{fx.package}/src/state.rs",
                 "use std::sync::OnceLock;\nstatic CACHE: OnceLock<String> = OnceLock::new();\n")
        self.check(fx, "lint-core-globals.sh", "lint-core-globals", "src/state.rs:CACHE — fixture: immutable data",
                   "src/state.rs:GONE — fixture: gone", f"{fx.package}/src/state.rs:GONE")

    def test_no_vendor(self):
        fx = self.fixture()
        fx.write(f"{fx.package}/templates/page.html", "<p>page</p>\n")
        self.check(fx, "check-no-vendor.sh", "check-no-vendor", "templates/*.html", "assets/*.js",
                   f"{fx.package}/assets/*.js")

    def test_table_ownership(self):
        fx = self.fixture("crates/app/demo")
        fx.write(f"{fx.package}/src/read.rs",
                 'pub async fn r() {\n    let _ = sqlx::query!("SELECT id FROM foreign_rows WHERE id = 1");\n}\n')
        self.check(fx, "lint-table-ownership.sh", "lint-table-ownership", "src/read.rs|foreign_rows|fixture read",
                   "src/read.rs|other_rows|fixture read", f"{fx.package}/src/read.rs|other_rows")

    def test_ledger_reads(self):
        fx = self.fixture()
        fx.write(f"{fx.package}/src/q.rs",
                 'pub async fn q() {\n    let _ = sqlx::query!("SELECT COUNT(*) FROM logs");\n}\n')
        listed = fx.run("lint-ledger-reads.sh", "--list")
        keys = [k for k in listed.stdout.splitlines() if k.startswith(fx.package + "/")]
        self.assertEqual(len(keys), 1, listed.stdout + listed.stderr)
        good = keys[0][len(fx.package) + 1:]
        self.check(fx, "lint-ledger-reads.sh", "lint-ledger-reads", good, "src/q.rs#0000000000",
                   f"{fx.package}/src/q.rs#0000000000")

    def test_misplaced_allowlist_stops_the_gate(self):
        fx = self.fixture()
        fx.write("crates/domain/nopackage/gates/lint-env-vars.allow", "src/a.rs  # x\n")
        result = fx.run("lint-env-vars.sh")
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("sits beside no package", result.stderr)


if __name__ == "__main__":
    unittest.main()
