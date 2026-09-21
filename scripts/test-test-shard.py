import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SHARD = ROOT / "scripts" / "test-shard.sh"


class TestShardInvocation(unittest.TestCase):
    def run_shard(self, group, *args, reuse=False):
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
                "elif [[ \" $* \" == *\" nextest run \"* ]]; then\n"
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


if __name__ == "__main__":
    unittest.main()
