#!/usr/bin/env python3
"""lint-contributions-surface refuses any Contributions method that returns
something other than the chaining, slot-builder or kernel-view forms, and
any pub field."""
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from fixture_tree import GateFixture  # noqa: E402

SOURCE = "crates/shared/extension/src/contribute/mod.rs"
STRUCT = "pub struct Contributions<'a> {\n    owner: ExtensionId,\n    sink: &'a mut ContributionSink,\n}\n"
GOOD = (
    "impl<'a> Contributions<'a> {\n"
    "    #[doc(hidden)]\n    pub const fn __new(owner: ExtensionId, sink: &'a mut ContributionSink) -> Self { todo!() }\n"
    "    pub const fn kernel(&self) -> &KernelView<'a> { &self.view }\n"
    "    pub fn schema(&mut self, s: SchemaDefinition) -> &mut Self { self }\n"
    "    pub fn slot<S: Slot>(&mut self) -> &mut S::Builder { todo!() }\n"
    "    pub fn provide<T, F>(&mut self, f: F) -> &mut Self\n    where\n        F: ProvideFn<T>,\n    { self }\n"
    "    fn helper(&mut self) -> &mut Vec<Seed> { &mut self.sink.seeds }\n"
    "}\n"
)


class ContributionsSurface(GateFixture):
    script = "lint-contributions-surface.py"

    def test_current_shape_passes(self):
        self.write(SOURCE, STRUCT + GOOD)
        self.check([])

    def test_entry_accessor_fails(self):
        bad = GOOD.replace("    fn helper", "    pub fn entries(&mut self) -> &mut Vec<Seed> { todo!() }\n    fn helper")
        self.write(SOURCE, STRUCT + bad)
        self.check([f"{SOURCE}:15"])

    def test_sink_accessor_fails(self):
        bad = GOOD.replace("    fn helper", "    pub fn sink(&mut self) -> &mut ContributionSink { self.sink }\n    fn helper")
        self.write(SOURCE, STRUCT + bad)
        self.check([f"{SOURCE}:15"])

    def test_pub_field_fails(self):
        self.write(SOURCE, STRUCT.replace("    sink:", "    pub sink:") + GOOD)
        self.check([f"{SOURCE}:3"])

    def test_missing_impl_fails_closed(self):
        self.write(SOURCE, STRUCT)
        self.check([SOURCE])


if __name__ == "__main__":
    unittest.main()
