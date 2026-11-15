#!/usr/bin/env python3
"""lint-orphan-tests reports exactly the files no crate root reaches.

Each test writes a crate tree into a temporary directory, runs the gate with
that directory as its ROOT and checks the reported set. The last test runs the
gate over the repository, which must report nothing.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-orphan-tests.py"


def run(*args):
    return subprocess.run(
        ["python3", str(SCRIPT), *args],
        cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
    )


class Tree:
    def __init__(self, base):
        self.base = Path(base)

    def write(self, rel, text=""):
        path = self.base / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def crate(self, rel, manifest_extra=""):
        self.write(f"{rel}/Cargo.toml", f'[package]\nname = "x"\nversion = "0.0.0"\n{manifest_extra}')


def reported(result, base):
    lines = [line.split(": ", 1)[0] for line in result.stdout.splitlines() if ": no crate root" in line]
    resolved = Path(base).resolve()
    return sorted(Path(line).resolve().relative_to(resolved).as_posix() for line in lines)


class OrphanTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.tree = Tree(self.tmp.name)
        self.tree.crate("a")

    def check(self, expected):
        result = run(self.tmp.name)
        self.assertEqual(reported(result, self.tmp.name), sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_orphan_is_found(self):
        self.tree.write("a/src/lib.rs", "mod used;\n")
        self.tree.write("a/src/used.rs")
        self.tree.write("a/src/lost.rs")
        self.check(["a/src/lost.rs"])

    def test_path_attribute_reaches_its_file(self):
        self.tree.write("a/src/lib.rs", '#[cfg(test)]\n#[path = "elsewhere/real.rs"]\npub(crate) mod alias;\n')
        self.tree.write("a/src/elsewhere/real.rs", "mod child;\n")
        self.tree.write("a/src/elsewhere/child.rs")
        self.tree.write("a/src/alias.rs")
        self.check(["a/src/alias.rs"])

    def test_nested_inline_module_adds_directory_levels(self):
        self.tree.write("a/src/lib.rs", "mod outer {\n    pub mod inner {\n        mod leaf;\n    }\n}\nmod top;\n")
        self.tree.write("a/src/outer/inner/leaf.rs")
        self.tree.write("a/src/top.rs", "mod deep { mod x; }\n")
        self.tree.write("a/src/top/deep/x.rs")
        self.tree.write("a/src/leaf.rs")
        self.check(["a/src/leaf.rs"])

    def test_mod_rs_form_and_non_mod_rs_children(self):
        self.tree.write("a/src/lib.rs", "mod group;\nmod flat;\n")
        self.tree.write("a/src/group/mod.rs", "mod member;\n")
        self.tree.write("a/src/group/member.rs")
        self.tree.write("a/src/flat.rs", "mod sub;\n")
        self.tree.write("a/src/flat/sub.rs")
        self.tree.write("a/src/sub.rs")
        self.check(["a/src/sub.rs"])

    def test_commented_out_or_quoted_mod_reaches_nothing(self):
        self.tree.write("a/src/lib.rs", (
            "// mod line;\n/* mod block; /* nested */ mod nested_block; */\n"
            'const S: &str = "mod quoted;";\nconst R: &str = r#"mod raw;"#;\n'
            "fn f<'a>(_: &'a str) -> char { '\"' }\nmod real;\n"
        ))
        for name in ("line", "block", "nested_block", "quoted", "raw", "real"):
            self.tree.write(f"a/src/{name}.rs")
        self.check(["a/src/block.rs", "a/src/line.rs", "a/src/nested_block.rs", "a/src/quoted.rs", "a/src/raw.rs"])

    def test_file_reached_only_from_another_crate_is_not_an_orphan(self):
        self.tree.write("a/src/lib.rs")
        self.tree.write("a/shared.rs")
        self.tree.crate("b")
        self.tree.write("b/src/lib.rs", '#[path = "../../a/shared.rs"]\nmod shared;\n')
        self.check([])

    def test_manifest_targets_and_auto_discovery(self):
        self.tree.crate("c", '\n[lib]\npath = "mod.rs"\n\n[[test]]\nname = "t"\npath = "suite/t.rs"\n')
        self.tree.write("c/mod.rs", "mod here;\n")
        self.tree.write("c/here.rs")
        self.tree.write("c/suite/t.rs")
        self.tree.write("c/tests/auto.rs")
        self.tree.write("c/build.rs")
        self.tree.write("c/stray.rs")
        self.tree.write("a/src/lib.rs")
        self.check(["c/stray.rs"])

    def test_nested_crate_files_belong_to_the_nested_crate(self):
        self.tree.write("a/src/lib.rs")
        self.tree.crate("a/inner")
        self.tree.write("a/inner/src/lib.rs")
        self.tree.write("a/inner/src/lost.rs")
        self.check(["a/inner/src/lost.rs"])

    def test_roots_and_paths_together_exit_2(self):
        result = run(self.tmp.name, "--paths", "crates")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("pass one, not both", result.stderr)


class Repository(unittest.TestCase):
    def test_repository_has_no_orphans(self):
        result = run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
