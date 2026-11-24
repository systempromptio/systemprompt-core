#!/usr/bin/env python3
"""lint-owner-witness refuses an id-only read named from a user-policy handler.

Each test writes a fixture entry crate whose `RouteSet` mounts handlers under
a user and an admin spec, runs the gate over it with `--root`, and checks the
findings; the last test runs the gate over the core tree.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-owner-witness.py"

MOUNT = textwrap.dedent("""\
    use systemprompt_extension::{AuthzPolicy, ContextKind, RouteSet, RouteSpec};

    const fn user_spec(limit: RateLimit) -> RouteSpec {
        RouteSpec::new(AuthzPolicy::user(), ContextKind::User, limit)
    }

    const USER: RouteSpec = user_spec(LIMIT);
    const ADMIN: RouteSpec = RouteSpec::new(AuthzPolicy::admin(), ContextKind::User, LIMITS);

    pub fn routes() -> RouteSet {
        RouteSet::at::<State>(BasePath::Api(ApiBase::Runs), RouteGroup::Agent)
            .route(Method::GET, "/{id}", handlers::get_run, USER)
            .route(
                Method::POST,
                "/simulate",
                admin::simulate,
                ADMIN,
            )
            .build()
    }
""")

OWNED = textwrap.dedent("""\
    pub async fn get_run(state: State, ctx: Ctx, id: Id) -> Response {
        state.runs.find_owned_run(&id, ctx.user_id()).await
    }
""")


class OwnerWitness(unittest.TestCase):
    def tree(self, files):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        src = Path(tmp.name) / "crates" / "entry" / "runs" / "src"
        files = {"http/mod.rs": MOUNT, "http/handlers.rs": OWNED,
                 "http/admin.rs": "pub async fn simulate() {}\n"} | files
        for rel, body in files.items():
            path = src / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(body)
        return tmp.name

    def run_gate(self, root, *args):
        return subprocess.run(
            ["python3", str(SCRIPT), *(["--root", root] if root else []), *args],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def findings(self, result):
        return [line.strip() for line in result.stdout.splitlines() if line.startswith("  ")]

    def test_owner_scoped_reads_pass(self):
        result = self.run_gate(self.tree({}))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("1 user-policy handler module", result.stdout)

    def test_id_only_read_in_a_user_handler_is_refused(self):
        body = "pub async fn get_run(state: State, id: Id) {\n    state.runs.find_run(&id).await\n}\n"
        result = self.run_gate(self.tree({"http/handlers.rs": body}))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(self.findings(result),
                         ["crates/entry/runs/src/http/handlers.rs:2: find_run reads by id alone"])

    def test_admin_handler_keeps_its_unscoped_read(self):
        body = "pub async fn simulate(repo: Repo, id: Id) {\n    repo.find_by_id(&id).await\n}\n"
        result = self.run_gate(self.tree({"http/admin.rs": body}))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_admin_repository_named_from_a_user_handler_is_refused(self):
        body = OWNED + "fn audit(repo: &RunAdminRepository) {}\n"
        result = self.run_gate(self.tree({"http/handlers.rs": body}))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("RunAdminRepository named from a user route", result.stdout)

    def test_reading_the_caller_from_the_user_directory_passes(self):
        body = OWNED + textwrap.dedent("""\
            pub async fn whoami(state: State, ctx: Ctx) {
                let me = state
                    .user_repository
                    .find_by_id(ctx.user_id())
                    .await;
            }
        """)
        result = self.run_gate(self.tree({"http/handlers.rs": body}))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_comment_naming_the_method_is_not_a_read(self):
        body = OWNED + "// find_run(&id) answers any owner\n"
        result = self.run_gate(self.tree({"http/handlers.rs": body}))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_an_unresolved_handler_fails_closed(self):
        result = self.run_gate(self.tree({"http/handlers.rs": "pub async fn other() {}\n"}))
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("handlers::get_run resolves to 0 file(s)", result.stderr)

    def test_core_tree_passes(self):
        result = self.run_gate(None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
