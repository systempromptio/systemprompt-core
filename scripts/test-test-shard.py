import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SHARD = ROOT / "scripts" / "test-shard.sh"


class TestShardInvocation(unittest.TestCase):
    def run_shard(self, group, *args, reuse=False, archive=False):
        with tempfile.TemporaryDirectory() as directory:
            temp = Path(directory)
            log = temp / "nextest-args"
            cargo = temp / "cargo"
            cargo.write_text(
                "#!/usr/bin/env bash\n"
                "set -e\n"
                "if [[ \" $* \" == *\" metadata \"* ]]; then\n"
                "  printf '%s\\n' '{\"packages\":[{\"name\":\"fixture-tests\",\"manifest_path\":\"/repo/crates/tests/integration/cli/Cargo.toml\"},{\"name\":\"bridge-fixture-tests\",\"manifest_path\":\"/repo/crates/tests/unit/bridge/Cargo.toml\"}]}'\n"
                "elif [[ \" $* \" == *\" --manifest-path bin/bridge/Cargo.toml \"* ]]; then\n"
                "  touch \"$BRIDGE_BUILD_MARKER\"\n"
                "elif [[ \" $* \" == *\" nextest \"* ]]; then\n"
                "  printf '%s\\n' \"$@\" > \"$SHARD_ARGS_LOG\"\n"
                "fi\n"
            )
            cargo.chmod(cargo.stat().st_mode | stat.S_IXUSR)
            executable = temp / "systemprompt"
            executable.touch()
            executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
            bridge = temp / "systemprompt-bridge"
            bridge.touch()
            bridge.chmod(bridge.stat().st_mode | stat.S_IXUSR)
            metadata = temp / "binaries.json"
            cargo_metadata = temp / "cargo-metadata.json"
            metadata.write_text("{}")
            cargo_metadata.write_text("{}")
            environment = os.environ | {
                "PATH": f"{temp}:{os.environ['PATH']}",
                "SYSTEMPROMPT_BIN": str(executable),
                "SP_BRIDGE_BIN": str(bridge),
                "SHARD_ARGS_LOG": str(log),
                "BRIDGE_BUILD_MARKER": str(temp / "bridge-built"),
                "TEST_THREADS": "1",
            }
            if archive:
                tests_archive = temp / "tests.tar.zst"
                tests_archive.touch()
                environment["NEXTEST_ARCHIVE_FILE"] = str(tests_archive)
            if reuse:
                environment |= {
                    "COVERAGE_BINARIES_METADATA": str(metadata),
                    "COVERAGE_CARGO_METADATA": str(cargo_metadata),
                }
            subprocess.run(
                ["bash", str(SHARD), group, *args],
                cwd=ROOT,
                env=environment,
                check=True,
                text=True,
                capture_output=True,
            )
            if reuse and group == "bridge":
                self.assertFalse((temp / "bridge-built").exists())
            return log.read_text().splitlines()

    def test_coverage_targets_are_unique_and_filter_is_preserved(self):
        args = self.run_shard(
            "integration-cli", "--bins", "--tests", "--filter-expr", "test(foo)",
        )
        self.assertEqual([flag for flag in args if flag in {
            "--lib", "--bins", "--tests", "--examples", "--benches",
        }], ["--lib", "--tests", "--bins"])
        self.assertEqual(args[-2:], ["--filter-expr", "test(foo)"])

    def test_cli_integration_targets_are_enabled_without_coverage_flags(self):
        args = self.run_shard("integration-cli")
        self.assertEqual(args.count("--lib"), 1)
        self.assertEqual(args.count("--tests"), 1)

    def test_every_shard_runs_integration_test_targets(self):
        args = self.run_shard("bridge")
        self.assertEqual(args.count("--lib"), 1)
        self.assertEqual(args.count("--tests"), 1)

    def test_arguments_after_separator_are_unchanged(self):
        args = self.run_shard("integration-cli", "--", "--tests")
        self.assertEqual(args[-2:], ["--", "--tests"])

    def test_metadata_reuse_selects_packages_without_cargo_target_flags(self):
        args = self.run_shard("integration-cli", "--no-fail-fast", reuse=True)
        self.assertEqual(args[args.index("--profile") + 1], "coverage")
        self.assertIn("-E", args)
        self.assertEqual(args[args.index("-E") + 1], "package(=fixture-tests)")
        self.assertNotIn("--manifest-path", args)
        self.assertNotIn("-p", args)
        self.assertNotIn("--lib", args)
        self.assertNotIn("--bins", args)
        self.assertNotIn("--tests", args)
        self.assertIn("--no-fail-fast", args)

    def test_metadata_filter_is_anded_and_prebuilt_bridge_is_reused(self):
        args = self.run_shard("bridge", "-E", "test(specific)", reuse=True)
        self.assertEqual(
            args[args.index("-E") + 1],
            "(package(=bridge-fixture-tests)) & (test(specific))",
        )

    def test_archive_run_selects_packages_and_remaps_the_workspace(self):
        args = self.run_shard("bridge", "-E", "test(specific)", archive=True)
        self.assertEqual(args[:2], ["nextest", "run"])
        self.assertEqual(args[args.index("--profile") + 1], "default")
        self.assertTrue(args[args.index("--archive-file") + 1].endswith("tests.tar.zst"))
        self.assertEqual(args[args.index("--workspace-remap") + 1], str(ROOT / "crates" / "tests"))
        self.assertEqual(
            args[args.index("-E") + 1],
            "(package(=bridge-fixture-tests)) & (test(specific))",
        )
        self.assertNotIn("--manifest-path", args)
        self.assertNotIn("--lib", args)
        self.assertNotIn("-p", args)

    def test_archive_build_covers_every_shard_package_once(self):
        args = self.run_shard("--archive", "out.tar.zst")
        self.assertEqual(args[:2], ["nextest", "archive"])
        self.assertEqual(args.count("-p"), 2)
        self.assertIn("fixture-tests", args)
        self.assertIn("bridge-fixture-tests", args)
        self.assertEqual(args.count("--lib"), 1)
        self.assertEqual(args.count("--tests"), 1)
        self.assertEqual(args[args.index("--archive-file") + 1], "out.tar.zst")

    def test_archive_set_builds_only_its_own_packages(self):
        args = self.run_shard("--archive", "out.tar.zst", "--set", "integration", "--locked")
        self.assertEqual(args[:2], ["nextest", "archive"])
        self.assertEqual(args.count("-p"), 1)
        self.assertIn("fixture-tests", args)
        self.assertNotIn("bridge-fixture-tests", args)
        self.assertIn("--locked", args)
        self.assertNotIn("--set", args)

    def test_every_shard_group_is_in_exactly_one_archive_set(self):
        def shard(*args):
            return subprocess.run(
                ["bash", str(SHARD), *args], cwd=ROOT, check=True, text=True, capture_output=True
            ).stdout.split()

        sets = shard("--list-archive-sets")
        self.assertGreater(len(sets), 1)
        for group in shard("--list"):
            self.assertIn(shard("--archive-set-of", group)[0], sets)

    def test_ci_builds_every_archive_set(self):
        sets = subprocess.run(
            ["bash", str(SHARD), "--list-archive-sets"], cwd=ROOT, check=True, text=True, capture_output=True
        ).stdout.split()
        workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        self.assertIn("        set: ${{ fromJSON(needs.affected.outputs.archive_sets) }}\n", workflow)
        self.assertIn("        group: ${{ fromJSON(needs.affected.outputs.shard_groups) }}\n", workflow)
        plan = subprocess.run(
            [sys.executable, "-B", str(ROOT / "scripts" / "ci-plan.py"), "plan", "--workflow", "ci", "--event",
             "pull_request", "--selection", "/dev/null", "--base", ""],
            cwd=ROOT, text=True, capture_output=True,
        )
        self.assertEqual(plan.returncode, 0, plan.stderr)
        outputs = dict(line.split("=", 1) for line in plan.stdout.splitlines())
        groups = subprocess.run(
            ["bash", str(SHARD), "--list"], cwd=ROOT, check=True, text=True, capture_output=True
        ).stdout.split()
        self.assertEqual(sorted(json.loads(outputs["archive_sets"])), sorted(sets))
        self.assertEqual(json.loads(outputs["shard_groups"]), groups)

    def test_unknown_group_has_no_archive_set(self):
        result = subprocess.run(
            ["bash", str(SHARD), "--archive-set-of", "nonesuch"], cwd=ROOT, text=True, capture_output=True
        )
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
