#!/usr/bin/env python3
"""The affected-set selector applies every rule to small synthetic graphs.

Pure functions take an injected graph, rule table, shard map and unit table.
One test reads the real repository: scripts/units.txt must name every package
outside the test workspace (`cargo metadata --offline --no-deps`).
"""
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("affected", ROOT / "scripts" / "affected.py")
affected = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(affected)


def node(name, workspace, *deps):
    return {"name": name, "workspace": workspace, "deps": list(deps)}


GRAPH = {
    "crates/shared/ids": node("ids", "."),
    "crates/infra/db": node("db", ".", ("crates/shared/ids", None)),
    "crates/domain/agent": node("agent", ".", ("crates/infra/db", None)),
    "crates/domain/ai": node("ai", ".", ("crates/shared/ids", "build")),
    "crates/domain/files": node("files", ".", ("crates/domain/agent", "dev")),
    "crates/entry/api": node("api", ".", ("crates/domain/agent", None), ("crates/domain/ai", None)),
    "bin/bridge": node("bridge", "bin/bridge", ("crates/shared/ids", None)),
    "crates/tests/bench": node("bench", "crates/tests/bench", ("crates/entry/api", None)),
    "crates/tests/common/hack": node("systemprompt-test-hack", "crates/tests", ("crates/domain/files", None)),
    "crates/tests/common/fixtures": node(
        "fixtures", "crates/tests", ("crates/domain/ai", None), ("crates/tests/common/hack", None)
    ),
    "crates/tests/unit/domain/agent": node(
        "agent-tests", "crates/tests", ("crates/domain/agent", "dev"), ("crates/tests/common/hack", None)
    ),
    "crates/tests/unit/domain/files": node(
        "files-tests", "crates/tests", ("crates/domain/files", None), ("crates/tests/common/hack", None)
    ),
    "crates/tests/unit/domain/ai": node("ai-tests", "crates/tests", ("crates/tests/common/fixtures", "dev")),
    "crates/tests/unit/bridge": node("bridge-tests", "crates/tests", ("bin/bridge", None)),
    "crates/tests/integration/api": node("api-it", "crates/tests", ("crates/entry/api", None)),
}

RULES = affected.parse_rules(
    "[full]\n"
    "**/rust-toolchain*\n**/clippy.toml\n**/.cargo/**\n**/Cargo.lock\nCargo.toml\n"
    ".github/workflows/**\nscripts/**\njustfile\ncrates/tests/common/**\n**/schema/**\n"
    "**/.sqlx/**\ncrates/shared/ids/**\n**/*.md\ndocumentation/**\n"
    "[none]\nLICENSE\n.gitignore\n"
)

SHARDS = {
    "groups": [
        ("domain", ["/tests/unit/domain/"]),
        ("bridge", ["/tests/unit/bridge/"]),
        ("integration-api", ["/tests/integration/api/"]),
    ],
    "archive_of": {"domain": "unit", "bridge": "bridge", "integration-api": "integration"},
}

UNITS = {
    "ids": "kernel",
    "db": "kernel",
    "api": "kernel",
    "agent": "capability:agents",
    "ai": "capability:ai",
    "files": "capability:files",
    "bridge": "capability:bridge",
    "bench": "tooling",
}


def run(*paths):
    return affected.select(list(paths), GRAPH, RULES, SHARDS, UNITS)


class OwningCrate(unittest.TestCase):
    def test_nearest_ancestor_package(self):
        self.assertEqual(affected.owning_package("crates/domain/agent/src/a/b.rs", GRAPH), "crates/domain/agent")

    def test_nested_workspace_wins_over_enclosing_directory(self):
        self.assertEqual(affected.owning_package("crates/tests/bench/benches/x.rs", GRAPH), "crates/tests/bench")
        self.assertEqual(affected.owning_package("bin/bridge/src/main.rs", GRAPH), "bin/bridge")

    def test_file_outside_any_crate(self):
        self.assertIsNone(affected.owning_package("crates/README", GRAPH))
        self.assertIsNone(affected.owning_package("LICENSE", GRAPH))


class Closure(unittest.TestCase):
    def test_reverse_closure_over_normal_edges(self):
        result = run("crates/infra/db/src/lib.rs")
        self.assertEqual(result["mode"], "affected")
        self.assertEqual(result["changed_crates"], ["db"])
        self.assertEqual(result["affected_crates"], ["agent", "api", "bench", "db"])
        self.assertEqual(result["clippy_packages"], ["agent", "api", "db"])

    def test_build_dependency_edges_propagate(self):
        result = affected.reverse_closure(GRAPH, {"crates/shared/ids"}, kinds=affected.PRODUCTION_KINDS)
        self.assertIn("crates/domain/ai", result)

    def test_dev_dependency_edges_do_not_propagate(self):
        self.assertNotIn("files", run("crates/domain/agent/src/lib.rs")["affected_crates"])

    def test_bridge_flag_follows_bridge_workspace(self):
        self.assertTrue(affected.select(["bin/bridge/src/main.rs"], GRAPH, RULES, SHARDS, UNITS)["bridge"])
        self.assertFalse(run("crates/domain/agent/src/lib.rs")["bridge"])


class TestCrates(unittest.TestCase):
    def test_direct_dependents_of_affected_crates_selected(self):
        result = run("crates/infra/db/src/lib.rs")
        self.assertEqual(result["test_packages"], ["agent-tests", "api-it"])

    def test_dependents_of_affected_test_support_selected(self):
        result = run("crates/domain/ai/src/lib.rs")
        self.assertEqual(result["test_packages"], ["ai-tests", "api-it", "fixtures"])

    def test_hack_crate_edges_ignored(self):
        result = run("crates/domain/files/src/lib.rs")
        self.assertEqual(result["test_packages"], ["files-tests"])
        self.assertNotIn("systemprompt-test-hack", result["test_packages"])

    def test_changed_test_crate_selects_itself(self):
        result = run("crates/tests/unit/domain/agent/src/lib.rs")
        self.assertEqual(result["affected_crates"], [])
        self.assertEqual(result["test_packages"], ["agent-tests"])
        self.assertEqual(result["scope_size"], 1)


class Shards(unittest.TestCase):
    def test_groups_and_archive_sets(self):
        result = run("crates/infra/db/src/lib.rs")
        self.assertEqual(result["shard_groups"], ["domain", "integration-api"])
        self.assertEqual(result["archive_sets"], ["integration", "unit"])

    def test_bridge_tests_map_to_bridge_shard(self):
        result = run("bin/bridge/src/lib.rs")
        self.assertEqual(result["shard_groups"], ["bridge"])
        self.assertEqual(result["archive_sets"], ["bridge"])

    def test_real_shard_map_covers_every_group(self):
        shard_map = affected.load_shard_map()
        listed = subprocess.run(
            ["bash", str(affected.TEST_SHARD), "--list"], capture_output=True, text=True, check=True
        ).stdout.split()
        self.assertEqual([g for g, _ in shard_map["groups"]], listed)
        for group, prefixes in shard_map["groups"]:
            self.assertTrue(prefixes, group)
            self.assertIn(shard_map["archive_of"][group], {"unit", "app", "bridge", "integration"})


class ForceFull(unittest.TestCase):
    CASES = {
        "rust-toolchain.toml": "**/rust-toolchain*",
        "bin/bridge/rust-toolchain.toml": "**/rust-toolchain*",
        "clippy.toml": "**/clippy.toml",
        ".cargo/config.toml": "**/.cargo/**",
        "crates/tests/Cargo.lock": "**/Cargo.lock",
        "Cargo.toml": "Cargo.toml",
        ".github/workflows/ci.yml": ".github/workflows/**",
        "scripts/lint-layers.sh": "scripts/**",
        "justfile": "justfile",
        "crates/tests/common/fixtures/src/db.rs": "crates/tests/common/**",
        "crates/infra/db/schema/migrations/001_x.sql": "**/schema/**",
        "crates/infra/db/.sqlx/query-1.json": "**/.sqlx/**",
        "crates/shared/ids/src/lib.rs": "crates/shared/ids/**",
        "crates/domain/agent/README.md": "**/*.md",
        "documentation/security.html": "documentation/**",
    }

    def test_each_pattern_class(self):
        for path, pattern in self.CASES.items():
            with self.subTest(path=path):
                result = run(path)
                self.assertEqual(result["mode"], "full")
                self.assertEqual(result["reasons"], [f"{path}: matches force-full pattern {pattern}"])
                self.assertEqual(result["shard_groups"], ["domain", "bridge", "integration-api"])
                self.assertTrue(result["bridge"])

    def test_full_reason_names_every_path(self):
        result = run("justfile", "Cargo.toml", "crates/domain/agent/src/lib.rs")
        self.assertEqual(len(result["reasons"]), 2)

    def test_unclassified_path_is_full(self):
        result = run(".gitleaks.toml")
        self.assertEqual(result["mode"], "full")
        self.assertEqual(result["reasons"], [".gitleaks.toml: owned by no crate and matched by no rule"])

    def test_glob_segments(self):
        self.assertIsNone(affected.first_match("a/b.toml", ["*.toml"]))
        self.assertEqual(affected.first_match("a/b/c.md", ["**/*.md"]), "**/*.md")
        self.assertEqual(affected.first_match("c.md", ["**/*.md"]), "**/*.md")

    def test_rules_reject_unknown_section_and_orphan_pattern(self):
        with self.assertRaises(affected.SelectorError):
            affected.parse_rules("[maybe]\nx\n")
        with self.assertRaises(affected.SelectorError):
            affected.parse_rules("x\n")

    def test_committed_table_parses(self):
        rules = affected.parse_rules(affected.FORCE_FULL.read_text())
        self.assertIn("**/Cargo.lock", rules["full"])
        self.assertIn("LICENSE", rules["none"])


class Modes(unittest.TestCase):
    def test_empty_diff_is_none(self):
        result = run()
        self.assertEqual(result["mode"], "none")
        self.assertEqual(result["scope_size"], 0)

    def test_none_section_only_is_none(self):
        self.assertEqual(run("LICENSE", ".gitignore")["mode"], "none")

    def test_rename_counts_both_sides(self):
        with tempfile.TemporaryDirectory() as directory:
            git = ["git", "-C", directory, "-c", "user.name=t", "-c", "user.email=t@t"]
            subprocess.run([*git, "init", "-q"], check=True)
            src = Path(directory) / "crates/domain/agent/src"
            src.mkdir(parents=True)
            (src / "old.rs").write_text("fn a() {}\n" * 20)
            subprocess.run([*git, "add", "."], check=True)
            subprocess.run([*git, "commit", "-qm", "a"], check=True)
            (Path(directory) / "crates/domain/ai/src").mkdir(parents=True)
            subprocess.run([*git, "mv", "crates/domain/agent/src/old.rs", "crates/domain/ai/src/new.rs"], check=True)
            subprocess.run([*git, "commit", "-qm", "b"], check=True)
            original = affected.ROOT
            affected.ROOT = Path(directory)
            try:
                changed = affected.changed_files("HEAD^", "HEAD")
            finally:
                affected.ROOT = original
        self.assertEqual(sorted(changed), ["crates/domain/agent/src/old.rs", "crates/domain/ai/src/new.rs"])
        self.assertEqual(run(*changed)["changed_crates"], ["agent", "ai"])


class Units(unittest.TestCase):
    def test_kernel_only_is_not_mixed(self):
        result = run("crates/infra/db/src/lib.rs")
        self.assertEqual(result["units"], ["kernel"])
        self.assertFalse(result["mixed_kernel_and_capability"])

    def test_kernel_and_capability_is_mixed(self):
        result = run("crates/infra/db/src/lib.rs", "crates/domain/ai/src/lib.rs")
        self.assertEqual(result["units"], ["capability:ai", "kernel"])
        self.assertTrue(result["mixed_kernel_and_capability"])

    def test_two_capabilities_are_not_mixed(self):
        self.assertFalse(run("crates/domain/ai/src/lib.rs", "crates/domain/agent/src/lib.rs")["mixed_kernel_and_capability"])

    def test_unclassified_crate_fails_closed(self):
        result = affected.select(["crates/infra/db/x.rs"], GRAPH, RULES, SHARDS, {})
        self.assertEqual(result["units"], ["unclassified"])
        self.assertTrue(result["mixed_kernel_and_capability"])

    def test_check_reports_missing_and_stale(self):
        units = dict(UNITS)
        del units["ai"]
        units["gone"] = "kernel"
        self.assertEqual(
            affected.check_units(GRAPH, units),
            ["ai: no unit in scripts/units.txt", "gone: listed in scripts/units.txt but no such package"],
        )
        self.assertEqual(affected.check_units(GRAPH, UNITS), [])

    def test_unit_table_rejects_bad_rows(self):
        for text in ("a kernel extra\n", "a capability:\n", "a kernel\na tooling\n", "a core\n"):
            with self.subTest(text=text), self.assertRaises(affected.SelectorError):
                affected.parse_units(text)

    def test_committed_table_names_every_workspace_package(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = affected.main(["--check"])
        self.assertEqual(code, 0, out.getvalue())


class Output(unittest.TestCase):
    def test_github_format_has_json_arrays(self):
        text = affected.render(run("crates/infra/db/src/lib.rs"), "github")
        self.assertIn('shard_groups=["domain","integration-api"]', text.splitlines())
        self.assertIn("bridge=false", text.splitlines())
        self.assertIn("mode=affected", text.splitlines())


if __name__ == "__main__":
    sys.exit(unittest.main())
