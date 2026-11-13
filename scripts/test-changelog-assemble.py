#!/usr/bin/env python3
"""Tests for scripts/changelog-assemble.py over a throwaway repository."""
import importlib.util
import io
import subprocess
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "changelog_assemble", Path(__file__).resolve().parent / "changelog-assemble.py")
ca = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ca)

ROOT_LOG = "# Changelog\n\n## [0.1.0] - 2026-01-01\n\n### Added\n\n- **Core:** first.\n"
CRATE_LOG = "# Changelog\n\n## [0.1.0] - 2026-01-01\n\n### Added\n\n- First.\n"


def manifest(name, extra=""):
    return f'[package]\nname = "{name}"\nversion.workspace = true\n{extra}\n[dependencies]\n'


class Repo:
    def __init__(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/a", "crates/b"]\n')
        self.write("CHANGELOG.md", ROOT_LOG)
        self.write("crates/a/Cargo.toml", manifest("crate-a"))
        self.write("crates/a/CHANGELOG.md", CRATE_LOG)
        self.write("crates/b/Cargo.toml", manifest("crate-b"))
        self.write("crates/private/Cargo.toml", manifest("crate-private", "publish = false"))
        self.write("crates/tests/unit/Cargo.toml", manifest("crate-tests"))
        (self.root / "changelog.d").mkdir()

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def fragment(self, fid, text):
        self.write(f"changelog.d/{fid}.toml", text)

    def read(self, rel):
        return (self.root / rel).read_text()

    def main(self, *argv):
        out = io.StringIO()
        real = ca.sys.stdout
        ca.sys.stdout = out
        try:
            code = ca.main(list(argv), root=self.root)
        finally:
            ca.sys.stdout = real
        return code, out.getvalue()

    def close(self):
        self.tmp.cleanup()


ROOT_AND_CRATE = '''[root]
section = "Breaking"
area = "Jobs"
text = "`submit_job!` takes the owning extension id."

[[crate]]
name = "crate-a"
section = "Breaking"
text = "`submit_job!` takes the extension id. Migrate by passing it."
'''


class Case(unittest.TestCase):
    def setUp(self):
        self.repo = Repo()
        self.addCleanup(self.repo.close)

    def problems(self):
        with self.assertRaises(ca.FragmentError) as raised:
            ca.load_fragments(self.repo.root, ca.published_crates(self.repo.root))
        return str(raised.exception)


class Crates(Case):
    def test_published_crates_skip_private_and_test_workspace(self):
        self.assertEqual(ca.published_crates(self.repo.root), {"crate-a": "crates/a", "crate-b": "crates/b"})


class Parse(unittest.TestCase):
    def test_escapes(self):
        root, crates = ca.parse_fragment('[[crate]]\nname = "x"\ntext = "a \\"b\\" \\\\ \\u00e9"\n', "f")
        self.assertIsNone(root)
        self.assertEqual(crates, [{"name": "x", "text": 'a "b" \\ é'}])

    def test_comment_after_string(self):
        _, crates = ca.parse_fragment('[[crate]]\nname = "x" # the crate\n', "f")
        self.assertEqual(crates[0]["name"], "x")

    def test_rejections(self):
        cases = {
            "[root]\n[root]\n": "at most one [root]",
            "[other]\n": "unknown table",
            'name = "x"\n': "outside a [root]",
            '[root]\nname = "x"\n': "unknown key 'name'",
            '[[crate]]\narea = "x"\n': "unknown key 'area'",
            '[root]\ntext = "a"\ntext = "b"\n': "duplicate key",
            "[root]\ntext = 'a'\n": "double-quoted",
            '[root]\ntext = "a\n': "unterminated",
            '[root]\ntext = "a" b\n': "trailing text",
            '[root]\ntext = "\\q"\n': "unsupported escape",
            "[root]\ntext\n": "expected",
        }
        for text, needle in cases.items():
            with self.subTest(text=text):
                with self.assertRaises(ca.FragmentError) as raised:
                    ca.parse_fragment(text, "f")
                self.assertIn(needle, str(raised.exception))


class Check(Case):
    def test_valid_fragments_pass(self):
        self.repo.fragment("a-change", ROOT_AND_CRATE)
        self.assertEqual(self.repo.main("--check"), (0, "changelog-assemble: 1 fragment(s) valid\n"))

    def test_no_fragments_is_valid(self):
        self.assertEqual(self.repo.main("--check")[0], 0)

    def test_every_problem_is_reported(self):
        self.repo.fragment("unknown", '[[crate]]\nname = "crate-private"\nsection = "Added"\ntext = "x"\n')
        self.repo.fragment("section", '[[crate]]\nname = "crate-a"\nsection = "Security"\ntext = "x"\n')
        self.repo.fragment("area", '[root]\nsection = "Added"\ntext = "x"\n')
        self.repo.fragment("empty", '[[crate]]\nname = "crate-a"\nsection = "Added"\ntext = "  "\n')
        self.repo.fragment("unreleased", '[[crate]]\nname = "crate-a"\nsection = "Added"\ntext = "see [Unreleased]"\n')
        self.repo.fragment("nothing", "# no entry\n")
        self.repo.fragment("Bad_Id", ROOT_AND_CRATE)
        problems = self.problems()
        for needle in ("unknown.toml: [[crate]] crate-private: not a published crate",
                       "section.toml: [[crate]] crate-a: section must be one of",
                       "area.toml: [root]: area is required",
                       "empty.toml: [[crate]] crate-a: text is empty",
                       "unreleased.toml: [[crate]] crate-a: fragments never name [Unreleased]",
                       "nothing.toml: no [root] or [[crate]] entry",
                       "Bad_Id.toml: id must be"):
            self.assertIn(needle, problems)
        self.assertEqual(self.repo.main("--check")[0], 1)

    def test_named_crates(self):
        self.repo.fragment("one", ROOT_AND_CRATE)
        self.repo.fragment("two", '[[crate]]\nname = "crate-b"\nsection = "Fixed"\ntext = "x"\n')
        self.assertEqual(self.repo.main("--named-crates"), (0, "crate-a\ncrate-b\n"))


class Assemble(Case):
    def fragments(self):
        self.repo.fragment("b-fix", '[root]\nsection = "Fixed"\narea = "Jobs"\ntext = "a fix."\n'
                           '\n[[crate]]\nname = "crate-a"\nsection = "Fixed"\ntext = "A fix."\n')
        self.repo.fragment("a-break", ROOT_AND_CRATE)
        self.repo.fragment("c-add", '[root]\nsection = "Added"\narea = "CLI"\ntext = "a command."\n'
                           '\n[[crate]]\nname = "crate-b"\nsection = "Added"\ntext = "B one."\n'
                           '\n[[crate]]\nname = "crate-b"\nsection = "Added"\ntext = "B two."\n')
        self.repo.fragment("a-add", '[[crate]]\nname = "crate-a"\nsection = "Added"\ntext = "A added."\n')

    def test_writes_house_format_sorted_and_consumes_fragments(self):
        self.fragments()
        code, out = self.repo.main("--version", "0.2.0", "--date", "2026-02-02")
        self.assertEqual(code, 0)
        self.assertIn("4 fragment(s) written to 3 changelog(s)", out)
        self.assertEqual(self.repo.read("CHANGELOG.md"), (
            "# Changelog\n\n## [0.2.0] - 2026-02-02\n\n"
            "### Breaking\n\n- **Breaking:** **Jobs:** `submit_job!` takes the owning extension id.\n\n"
            "### Added\n\n- **CLI:** a command.\n\n"
            "### Fixed\n\n- **Jobs:** a fix.\n\n"
            "## [0.1.0] - 2026-01-01\n\n### Added\n\n- **Core:** first.\n"))
        self.assertEqual(self.repo.read("crates/a/CHANGELOG.md"), (
            "# Changelog\n\n## [0.2.0] - 2026-02-02\n\n"
            "### Breaking\n\n- `submit_job!` takes the extension id. Migrate by passing it.\n\n"
            "### Added\n\n- A added.\n\n"
            "### Fixed\n\n- A fix.\n\n"
            "## [0.1.0] - 2026-01-01\n\n### Added\n\n- First.\n"))
        self.assertEqual(self.repo.read("crates/b/CHANGELOG.md"),
                         "# Changelog\n\n## [0.2.0] - 2026-02-02\n\n### Added\n\n- B one.\n- B two.\n")
        self.assertEqual(list((self.repo.root / "changelog.d").iterdir()), [])

    def test_dry_run_prints_and_changes_nothing(self):
        self.fragments()
        before = self.repo.read("CHANGELOG.md")
        code, out = self.repo.main("--version", "0.2.0", "--date", "2026-02-02", "--dry-run")
        self.assertEqual(code, 0)
        self.assertEqual([line for line in out.splitlines() if line.startswith("==> ")],
                         ["==> CHANGELOG.md", "==> crates/a/CHANGELOG.md", "==> crates/b/CHANGELOG.md"])
        self.assertEqual(self.repo.read("CHANGELOG.md"), before)
        self.assertEqual(len(list((self.repo.root / "changelog.d").iterdir())), 4)

    def test_output_is_byte_stable(self):
        self.fragments()
        first = self.repo.main("--version", "0.2.0", "--date", "2026-02-02", "--dry-run")
        self.assertEqual(first, self.repo.main("--version", "0.2.0", "--date", "2026-02-02", "--dry-run"))

    def test_existing_heading_refuses_and_writes_nothing(self):
        self.fragments()
        self.repo.write("crates/b/CHANGELOG.md", "# Changelog\n\n## [0.2.0] - 2026-02-01\n")
        before = self.repo.read("CHANGELOG.md")
        self.assertEqual(self.repo.main("--version", "0.2.0", "--date", "2026-02-02")[0], 1)
        self.assertEqual(self.repo.read("CHANGELOG.md"), before)
        self.assertEqual(len(list((self.repo.root / "changelog.d").iterdir())), 4)

    def test_invalid_fragment_writes_nothing(self):
        self.fragments()
        self.repo.fragment("z-bad", '[[crate]]\nname = "nope"\nsection = "Added"\ntext = "x"\n')
        before = self.repo.read("CHANGELOG.md")
        self.assertEqual(self.repo.main("--version", "0.2.0", "--date", "2026-02-02")[0], 1)
        self.assertEqual(self.repo.read("CHANGELOG.md"), before)

    def test_no_fragments_refuses(self):
        self.assertEqual(self.repo.main("--version", "0.2.0", "--date", "2026-02-02")[0], 1)

    def test_bad_version_or_date_is_a_usage_error(self):
        for argv in (["--version", "0.2", "--date", "2026-02-02"], ["--version", "0.2.0", "--date", "today"],
                     ["--version", "0.2.0"], ["--check", "--dry-run"]):
            with self.subTest(argv=argv):
                with self.assertRaises(SystemExit) as raised:
                    self.repo.main(*argv)
                self.assertEqual(raised.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
