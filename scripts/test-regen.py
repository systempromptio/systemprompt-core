#!/usr/bin/env python3
"""Tests for scripts/regen.sh: argument handling and the order of its plan."""
import os
import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REGEN = ROOT / "scripts" / "regen.sh"
DB = "postgres://u@localhost:5432/regen_probe"


def regen(*args):
    env = {**os.environ, "TEST_DATABASE_URL": DB}
    return subprocess.run(["bash", str(REGEN), *args], cwd=ROOT, env=env, capture_output=True, text=True)


def plan(*args):
    result = regen("--dry-run", *args)
    if result.returncode != 0:
        raise AssertionError(result.stderr)
    return [tuple(line.split(None, 1)) for line in result.stdout.splitlines()]


class Plan(unittest.TestCase):
    def test_phases_run_in_order(self):
        phases = [phase for phase, _ in plan()]
        order = ["migrate", "sqlx", "hakari", "lockfiles", "proof"]
        self.assertEqual(sorted(set(phases), key=order.index), order)
        self.assertEqual(phases, sorted(phases, key=order.index))

    def test_migrate_builds_online_before_any_cache_is_touched(self):
        steps = plan()
        self.assertEqual(steps[0][0], "migrate")
        self.assertIn("scripts/test-db.sh --online", steps[0][1])

    def test_sqlx_targets_the_test_database_and_audits(self):
        commands = [command for phase, command in plan() if phase == "sqlx"]
        self.assertEqual(len(commands), 3)
        self.assertTrue(commands[0].endswith("sqlx-prepare.sh workspace"))
        self.assertTrue(commands[1].endswith("sqlx-prepare.sh publish"))
        self.assertEqual(commands[2], "bash scripts/sqlx-audit-caches.sh")
        for command in commands[:2]:
            self.assertIn(f"DATABASE_URL={DB}", command)
            self.assertNotIn("PREPARE_ALLOW_PRUNE", command)

    def test_allow_prune_is_explicit(self):
        commands = [command for phase, command in plan("--allow-prune") if phase == "sqlx"]
        for command in commands[:2]:
            self.assertIn("PREPARE_ALLOW_PRUNE=1", command)

    def test_every_justfile_workspace_gets_its_lockfile(self):
        text = (ROOT / "justfile").read_text()
        workspaces = re.search(r'^workspaces\s*:=\s*"([^"]*)"', text, re.M).group(1).split()
        commands = [command for phase, command in plan() if phase == "lockfiles"]
        self.assertEqual(commands, [f"cargo metadata --format-version 1 --manifest-path {ws}/Cargo.toml"
                                    for ws in workspaces])

    def test_proof_is_last_unless_skipped(self):
        self.assertEqual(plan()[-1], ("proof", "just build-offline"))
        self.assertNotIn("proof", [phase for phase, _ in plan("--no-proof")])

    def test_unknown_argument_is_a_usage_error(self):
        result = regen("--yes")
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage:", result.stderr)


if __name__ == "__main__":
    unittest.main()
