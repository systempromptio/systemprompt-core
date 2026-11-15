#!/usr/bin/env python3
"""lint-kernel-identity reports type names, owner literals and string id
comparisons in the api mount and cli kernel only."""
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from fixture_tree import GateFixture  # noqa: E402

RESERVED = (
    "impl CoreId {\n    pub const fn id(self) -> &'static str {\n        match self {\n"
    "            Self::AnalyticsHttp => \"analytics-http\",\n            Self::Cloud => \"cloud\",\n"
    "        }\n    }\n}\n"
)
MOUNT = "crates/entry/api/src/services/middleware/mount/recheck.rs"
REGISTRY = "crates/entry/cli/src/kernel/registry.rs"


class KernelIdentity(GateFixture):
    script = "lint-kernel-identity.py"

    def setUp(self):
        super().setUp()
        self.write("crates/shared/extension/src/reserved/mod.rs", RESERVED)

    def test_typed_comparison_passes(self):
        self.write(MOUNT, "fn ok(o: &ExtensionId, e: CoreId) -> bool { o.as_str() == e.id() }\n")
        self.check([])

    def test_type_name_fails(self):
        self.write(MOUNT, "fn f() {\n    std::any::type_name::<T>();\n}\n")
        self.check([f"{MOUNT}:2"])

    def test_owner_literal_fails(self):
        self.write(REGISTRY, "const OWNER: &str = \"cloud\";\n")
        self.check([f"{REGISTRY}:1"])

    def test_string_comparison_fails(self):
        self.write(REGISTRY, "fn f(o: &ExtensionId) -> bool { o.as_str() == \"my-ext\" }\n")
        self.check([f"{REGISTRY}:1"])

    def test_outside_kernel_trees_passes(self):
        self.write("crates/entry/cli/src/commands/x.rs", "const OWNER: &str = \"cloud\";\nfn f() { type_name::<T>() }\n")
        self.write(MOUNT, "// type_name \"analytics-http\"\n")
        self.check([])

    def test_missing_core_ids_fails_closed(self):
        self.write("crates/shared/extension/src/reserved/mod.rs", "pub enum CoreId {}\n")
        self.write(MOUNT, "fn f() {}\n")
        self.check(["crates/shared/extension/src/reserved/mod.rs"])


if __name__ == "__main__":
    unittest.main()
