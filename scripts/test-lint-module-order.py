#!/usr/bin/env python3
"""lint-module-order reports every path that names a later module of the engine
or harness crate, every unplaced module and every missing one, and nothing in
comments, strings, lib.rs or paths that stay inside a module."""
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from fixture_tree import GateFixture  # noqa: E402

ENGINE = "crates/domain/engine/src"
HARNESS = "crates/app/harness/src"
ENGINE_ORDER = ("config", "store", "llm", "tools", "mcp", "workspace", "lsp", "prompt", "run")
HARNESS_ORDER = ("projection", "extension", "governance", "members", "sync", "command")


class ModuleOrder(GateFixture):
    script = "lint-module-order.py"

    def setUp(self):
        super().setUp()
        for src, order in ((ENGINE, ENGINE_ORDER), (HARNESS, HARNESS_ORDER)):
            self.write(f"{src}/lib.rs", "".join(f"pub mod {name};\n" for name in order))
            for name in order:
                self.write(f"{src}/{name}/mod.rs", "pub fn f() {}\n")

    def test_the_skeleton_passes(self):
        self.check([])

    def test_earlier_modules_and_the_own_module_pass(self):
        self.write(f"{ENGINE}/run/mod.rs", "use crate::config::A;\nuse crate::{store::B, prompt, run::tools::C};\n")
        self.write(f"{ENGINE}/tools/a.rs", "use crate::tools::b::X;\nuse super::b::Y;\nfn f() { crate::config::g(); }\n")
        self.write(f"{HARNESS}/sync/mod.rs", "use crate::{projection::P, extension, governance::G, members};\n")
        self.check([])

    def test_every_later_module_is_refused_in_each_crate(self):
        for src, order in ((ENGINE, ENGINE_ORDER), (HARNESS, HARNESS_ORDER)):
            for index, own in enumerate(order[:-1]):
                later = order[index + 1]
                with self.subTest(own=own, later=later):
                    self.write(f"{src}/{own}/mod.rs", f"pub fn f() {{}}\nuse crate::{later}::X;\n")
                    self.check([f"{src}/{own}/mod.rs:2"])
                    self.write(f"{src}/{own}/mod.rs", "pub fn f() {}\n")

    def test_paths_inside_expressions_types_and_macros_fail(self):
        self.write(
            f"{ENGINE}/config/a.rs",
            "fn f() -> crate::run::R {\n    crate::tools::g()\n}\nmacro_rules! m { () => { $crate::llm::X } }\n",
        )
        self.check([f"{ENGINE}/config/a.rs:{n}" for n in (1, 2, 4)])

    def test_groups_are_read_across_lines_and_nesting(self):
        self.write(
            f"{ENGINE}/store/a.rs",
            "use crate::{\n    config::A,\n    run::{self, tools::B},\n    store::{run, llm},\n    prompt,\n};\n",
        )
        self.check([f"{ENGINE}/store/a.rs:{n}" for n in (3, 5)])

    def test_super_chains_that_reach_the_root_fail(self):
        self.write(f"{ENGINE}/llm/mod.rs", "use super::run::X;\nuse super::config::Y;\n")
        self.write(f"{ENGINE}/llm/a/b.rs", "use super::super::super::mcp::X;\nuse super::super::run_local::Y;\n")
        self.write(f"{ENGINE}/llm/c.rs", "use super::run::Z;\nmod inner {\n    use super::super::run::W;\n    use super::super::super::run::V;\n}\nuse super::super::tools::T;\n")
        self.check([f"{ENGINE}/llm/mod.rs:1", f"{ENGINE}/llm/a/b.rs:1", f"{ENGINE}/llm/c.rs:4", f"{ENGINE}/llm/c.rs:6"])

    def test_the_crates_own_name_is_a_root(self):
        self.write(f"{ENGINE}/config/a.rs", "use systemprompt_engine::run::X;\nuse ::systemprompt_engine::store::Y;\nuse systemprompt_harness::sync::Z;\n")
        self.write(f"{HARNESS}/projection/a.rs", "use systemprompt_harness::sync::Z;\nuse systemprompt_engine::run::X;\n")
        self.check([f"{ENGINE}/config/a.rs:1", f"{ENGINE}/config/a.rs:2", f"{HARNESS}/projection/a.rs:1"])

    def test_a_submodule_named_like_a_later_module_passes(self):
        self.write(f"{ENGINE}/prompt/mod.rs", "mod run;\nuse self::run::X;\nuse run::Y;\nuse crate::prompt::run::Z;\n")
        self.write(f"{ENGINE}/prompt/run.rs", "use super::tools::X;\n")
        self.check([])

    def test_members_share_one_place(self):
        self.write(f"{HARNESS}/members/drive/mod.rs", "use crate::members::brain::B;\nuse crate::governance::G;\n")
        self.write(f"{HARNESS}/members/brain/a.rs", "use crate::sync::S;\n")
        self.write(f"{HARNESS}/governance/a.rs", "use crate::members::REGISTERED;\n")
        self.check([f"{HARNESS}/members/brain/a.rs:1", f"{HARNESS}/governance/a.rs:1"])

    def test_comments_strings_and_lib_pass(self):
        self.write(
            f"{ENGINE}/config/a.rs",
            "//! See [`crate::run::Engine`].\n// crate::run::x\n/* crate::tools */\nconst S: &str = \"crate::run::x\";\npub(crate) fn f() {}\n",
        )
        self.write(f"{ENGINE}/lib.rs", "pub mod config;\npub use crate::run::X;\n")
        self.check([])

    def test_a_module_without_a_place_fails(self):
        self.write(f"{ENGINE}/extra.rs", "pub fn f() {}\n")
        self.write(f"{HARNESS}/update/mod.rs", "pub fn f() {}\n")
        self.write(f"{HARNESS}/update/a.rs", "pub fn f() {}\n")
        self.check([f"{ENGINE}/extra.rs:1", f"{HARNESS}/update/mod.rs:1", f"{HARNESS}/update/a.rs:1"])

    def test_a_missing_module_fails(self):
        (self.base / ENGINE / "lsp" / "mod.rs").unlink()
        (self.base / ENGINE / "lsp").rmdir()
        result = self.run_gate()
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn(f"  {ENGINE}: no module `lsp`", result.stdout)

    def test_a_missing_crate_fails(self):
        for name in HARNESS_ORDER:
            (self.base / HARNESS / name / "mod.rs").unlink()
            (self.base / HARNESS / name).rmdir()
        (self.base / HARNESS / "lib.rs").unlink()
        (self.base / HARNESS).rmdir()
        result = self.run_gate()
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertEqual(result.stdout.count(f"  {HARNESS}: no module"), len(HARNESS_ORDER), result.stdout)

    def test_a_module_as_a_file_is_judged(self):
        (self.base / ENGINE / "config" / "mod.rs").unlink()
        (self.base / ENGINE / "config").rmdir()
        self.write(f"{ENGINE}/config.rs", "use crate::store::S;\nuse super::llm::L;\n")
        self.check([f"{ENGINE}/config.rs:1", f"{ENGINE}/config.rs:2"])


if __name__ == "__main__":
    unittest.main()
