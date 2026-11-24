#!/usr/bin/env python3
"""lint-hidden-constructors reports exactly the calls and re-exports it bans.

Each test writes a repository tree into a temporary directory and runs the
gate with that directory as its ROOT.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-hidden-constructors.py"

API_LIB = (
    "pub fn mount_admitted(tree: Router, admitted: &Admitted) -> Router { tree }\n"
    "pub fn protect(router: Router) -> Router { router }\n"
)
CLI_LIB = "pub fn run_composed<A>(declared: DeclaredCommands, admit: A) -> u8 { 0 }\n"


class HiddenConstructors(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.write("crates/entry/api/src/lib.rs", API_LIB)
        self.write("crates/entry/cli/src/lib.rs", CLI_LIB)
        self.write(
            "crates/shared/extension/src/admitted.rs",
            "impl Admitted {\n    pub fn __from_facade(parts: AdmittedParts) -> Self { todo() }\n}\n",
        )
        self.write("systemprompt/src/composition.rs", "fn admit() { Admitted::__from_facade(parts) }\n")

    def write(self, rel, text):
        path = self.base / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def check(self, expected):
        result = subprocess.run(
            ["python3", str(SCRIPT), str(self.base)],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )
        found = sorted(line.strip().split(": ", 1)[0] for line in result.stdout.splitlines() if line.startswith("  "))
        self.assertEqual(found, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_owners_and_definitions_pass(self):
        self.write("crates/entry/api/src/gateway.rs", "fn verify(u: UserId) { GatewayPrincipal::__from_kernel(u); }\n")
        self.check([])

    def test_facade_call_outside_facade_fails(self):
        self.write("crates/entry/cli/src/forge.rs", "fn f() {\n    DeclaredCommands::__from_facade(p);\n}\n")
        self.check(["crates/entry/cli/src/forge.rs:2"])

    def test_kernel_call_outside_api_fails(self):
        self.write("crates/entry/notifications/src/h.rs", "fn f(u: UserId) { GatewayPrincipal::__from_kernel(u) }\n")
        self.check(["crates/entry/notifications/src/h.rs:1"])

    def test_hop_caller_in_the_toolset_host_passes(self):
        self.write("crates/app/mcp-host/src/hop.rs", "fn admit(c: RequestContext) { VerifiedCaller::__from_hop(c); }\n")
        self.check([])

    def test_hop_caller_built_by_a_member_fails(self):
        self.write("crates/domain/brain/src/mcp/forge.rs", "fn f(c: RequestContext) { VerifiedCaller::__from_hop(c) }\n")
        self.check(["crates/domain/brain/src/mcp/forge.rs:1"])

    def test_comment_and_string_are_not_calls(self):
        self.write("crates/app/runtime/src/x.rs", "// Admitted::__from_facade(p)\nconst S: &str = \"__from_facade(\";\n")
        self.check([])

    def test_test_workspace_is_not_scanned(self):
        self.write("crates/tests/unit/systemprompt/src/forged.rs", "fn f() { Admitted::__from_facade(p); }\n")
        self.check([])

    def test_named_handoff_reexport_fails(self):
        self.write("systemprompt/src/lib.rs", "pub mod cli {\n    pub use systemprompt_cli::{CliRunError, run_composed};\n}\n")
        self.check(["systemprompt/src/lib.rs:2"])

    def test_glob_and_alias_reexports_fail(self):
        self.write("systemprompt/src/lib.rs", "pub use systemprompt_api::*;\npub use systemprompt_cli as cli;\n")
        self.check(["systemprompt/src/lib.rs:1", "systemprompt/src/lib.rs:2"])

    def test_unrelated_reexport_passes(self):
        self.write("systemprompt/src/lib.rs", "pub use systemprompt_api::protect;\npub use systemprompt_cli::CliRunError;\n")
        self.check([])


if __name__ == "__main__":
    unittest.main()
