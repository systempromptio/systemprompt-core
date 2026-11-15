#!/usr/bin/env python3
"""lint-no-inventory reports every name of the inventory crate in the root
workspace, and nothing in comments, strings, crates/tests or bin/bridge."""
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from fixture_tree import GateFixture  # noqa: E402


class NoInventory(GateFixture):
    script = "lint-no-inventory.py"

    def test_clean_tree_passes(self):
        self.write("crates/shared/extension/src/lib.rs", "pub fn f() {}\n")
        self.check([])

    def test_paths_use_and_extern_fail(self):
        self.write(
            "crates/shared/extension/src/lib.rs",
            "inventory::collect!(X);\nuse inventory;\nextern crate inventory;\npub use inventory as __inventory;\n",
        )
        self.check([f"crates/shared/extension/src/lib.rs:{n}" for n in (1, 2, 3, 4)])

    def test_facade_is_scanned(self):
        self.write("systemprompt/src/lib.rs", "fn f() { for x in inventory::iter::<X> {} }\n")
        self.check(["systemprompt/src/lib.rs:1"])

    def test_comments_strings_and_other_workspaces_pass(self):
        self.write("crates/app/runtime/src/a.rs", "// inventory::submit!\nconst S: &str = \"inventory::iter\";\nlet inventory = 1;\n")
        self.write("crates/tests/unit/x/src/lib.rs", "inventory::submit! {}\n")
        self.write("bin/bridge/src/main.rs", "inventory::collect!(Y);\n")
        self.check([])


if __name__ == "__main__":
    unittest.main()
