#!/usr/bin/env python3
"""scripts/gate.sh plans the right phases and commands for every mode.

Plans are built from synthetic selector output; the end-to-end cases run
`gate.sh --dry-run --selection FILE`, which prints the plan and runs nothing.
"""
from datetime import datetime, timedelta, timezone
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts" / "lib"))
import gate_runner  # noqa: E402
import gate_scope  # noqa: E402

AFFECTED = {
    "mode": "affected", "reasons": [], "changed_crates": ["systemprompt-files"],
    "affected_crates": ["systemprompt-api", "systemprompt-files"],
    "affected_dirs": ["crates/domain/files", "crates/entry/api"],
    "clippy_packages": ["systemprompt-api", "systemprompt-files"],
    "test_packages": ["systemprompt-files-tests"], "test_dirs": ["crates/tests/unit/domain/files"],
    "shard_groups": ["domain"], "archive_sets": ["unit"], "bridge": False,
    "units": ["capability:files"], "mixed_kernel_and_capability": False, "scope_size": 3,
}
NONE = dict(AFFECTED, mode="none", changed_crates=[], affected_crates=[], affected_dirs=[], clippy_packages=[],
            test_packages=[], test_dirs=[], shard_groups=[], archive_sets=[], units=[], scope_size=0)
FORCED = dict(AFFECTED, mode="full", reasons=["Cargo.lock: matches force-full pattern **/Cargo.lock"],
              shard_groups=["shared", "domain"])


URL = "postgres://u@h:5432/gate_test"
LIVE = f"DATABASE_URL='{URL}' SQLX_OFFLINE='false' TEST_DATABASE_URL='{URL}'"
PREP = f"TEST_DATABASE_URL='{URL}' bash scripts/test-db.sh"


def shard(group):
    return [f"{LIVE} bash scripts/test-db.sh", f"{LIVE} bash scripts/test-shard.sh {group}"]


def commands(phases):
    return {name: [gate_runner.render_step(entry) for entry in steps] for name, steps in phases}


def dry_run(selection, *args, env=None):
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
        json.dump(selection, handle)
    try:
        return subprocess.run(["bash", str(ROOT / "scripts" / "gate.sh"), *args, "--selection", handle.name,
                               "--dry-run"], capture_output=True, text=True, cwd=ROOT,
                              env=os.environ | (env or {}))
    finally:
        Path(handle.name).unlink()


class Plans(unittest.TestCase):
    def test_affected_plan_scopes_every_phase(self):
        plan = commands(gate_runner.plan(AFFECTED, ["crates/domain/files/src/lib.rs"], db_url=URL))
        self.assertEqual(list(plan), ["db", *gate_runner.PHASES])
        self.assertEqual(plan["db"], [PREP])
        self.assertEqual(plan["fmt"], ["just format-check"])
        self.assertEqual(plan["clippy"], [
            "cargo clippy -p systemprompt-api -p systemprompt-files --all-targets --all-features --keep-going -- -D warnings",
            f"(cd crates/tests && {LIVE} cargo clippy -p systemprompt-files-tests --all-targets --all-features --keep-going "
            "-- -D warnings)"])
        self.assertEqual(plan["doc"], [
            "RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features -p systemprompt-api -p systemprompt-files",
            f"(cd crates/tests && DATABASE_URL='{URL}' RUSTDOCFLAGS='-D warnings' SQLX_OFFLINE='false' "
            f"TEST_DATABASE_URL='{URL}' cargo doc --no-deps -p systemprompt-files-tests)"])
        self.assertEqual(plan["scripts"], [])
        self.assertEqual(plan["tests"], shard("domain"))

    def test_unit_gates_take_the_affected_dirs_and_global_gates_run_whole(self):
        plan = gate_runner.plan(AFFECTED, [], ["gates"])
        steps = {entry["label"]: entry["argv"] for entry in plan[0][1]}
        table = gate_scope.gate_table()
        self.assertEqual(list(steps), gate_scope.gate_list())
        paths = ["--paths", "crates/domain/files", "crates/entry/api", "crates/tests/unit/domain/files"]
        for gate, argv in steps.items():
            expected = ["just", gate] + (paths if table[gate]["scope"] == "unit" else [])
            self.assertEqual(argv, expected, gate)

    def test_bridge_adds_its_own_clippy_and_doc(self):
        plan = commands(gate_runner.plan(dict(AFFECTED, bridge=True), [], ["clippy", "doc"], db_url=URL))
        self.assertIn("just lint-bridge", plan["clippy"])
        self.assertIn("RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path bin/bridge/Cargo.toml --no-deps", plan["doc"])

    def test_none_mode_keeps_only_global_gates_and_fmt(self):
        plan = commands(gate_runner.plan(NONE, ["LICENSE"]))
        self.assertEqual(plan["clippy"], [])
        self.assertEqual(plan["tests"], [])
        globals_ = [g for g, row in gate_scope.gate_table().items() if row["scope"] == "global"]
        self.assertEqual(sorted(c.split()[1] for c in plan["gates"]), sorted(globals_))

    def test_script_changes_run_script_tests_and_rust_contracts_changes_their_tests(self):
        plan = commands(gate_runner.plan(NONE, ["scripts/rust-contracts/src/main.rs"], ["gates", "scripts"]))
        self.assertEqual(plan["scripts"], ["bash scripts/test-scripts.sh"])
        self.assertEqual(plan["gates"][-1], "cargo test --locked --manifest-path scripts/rust-contracts/Cargo.toml")

    def test_full_plan_is_check_gates_plus_everything(self):
        plan = commands(gate_runner.plan(gate_runner.full_selection(["shared", "edge"]), [], db_url=URL))
        self.assertEqual(plan["gates"], [f"just {g}" for g in gate_scope.gate_list()]
                         + ["cargo test --locked --manifest-path scripts/rust-contracts/Cargo.toml"])
        self.assertEqual(plan["clippy"][0], "just lint")
        self.assertEqual(plan["doc"], [f"{LIVE} just doc-check"])
        self.assertEqual(plan["scripts"], ["bash scripts/test-scripts.sh"])
        self.assertEqual(plan["tests"], shard("shared") + shard("edge"))
        self.assertEqual(plan["clippy"][1], f"(cd crates/tests && {LIVE} cargo clippy --workspace --all-targets "
                         "--all-features --keep-going -- -D warnings)")


class Groups(unittest.TestCase):
    def test_groups_partition_the_gate_list(self):
        table = gate_scope.gate_table()
        seen = []
        for group in gate_runner.GROUPS:
            plan = commands(gate_runner.plan(gate_runner.full_selection([]), [], ["gates"], group=group))
            gates = [c.split()[1] for c in plan["gates"] if c.startswith("just ")]
            self.assertTrue(gates, group)
            self.assertTrue(all(table[g]["group"] == group for g in gates), group)
            seen += gates
            has_contracts = any("rust-contracts" in c for c in plan["gates"])
            self.assertEqual(has_contracts, group == gate_runner.RUST_CONTRACTS_GROUP, group)
        self.assertEqual(sorted(seen), sorted(gate_scope.gate_list()))

    def test_cost_keeps_only_gates_of_that_cost(self):
        table = gate_scope.gate_table()
        plan = commands(gate_runner.plan(gate_runner.full_selection([]), [], ["fmt", "gates"], costs={"cheap"}))
        gates = [c.split()[1] for c in plan["gates"]]
        self.assertEqual(sorted(gates), sorted(g for g, row in table.items() if row["cost"] == "cheap"))
        self.assertNotIn("lint-discarded-results", gates)
        self.assertNotIn("lint-fail-open", gates)
        self.assertFalse(any("rust-contracts" in c for c in plan["gates"]))
        self.assertTrue(plan["fmt"])

    def test_cost_names_are_checked(self):
        self.assertIsNone(gate_runner.choose_costs(None))
        self.assertEqual(gate_runner.choose_costs("cheap,db"), {"cheap", "db"})
        with self.assertRaises(gate_runner.GateError):
            gate_runner.choose_costs("fast")

    def test_machete_is_a_source_gate(self):
        self.assertIn("machete", gate_scope.gate_list())
        self.assertEqual(gate_scope.gate_table()["machete"]["group"], "release")

    def test_unknown_group_is_refused(self):
        with self.assertRaises(gate_runner.GateError):
            gate_runner.plan(AFFECTED, [], ["gates"], group="nope")

    def test_group_dry_run(self):
        result = dry_run(AFFECTED, "--full", "--only", "gates", "--group", "release")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("  group release", result.stdout)
        self.assertIn("  just machete", result.stdout)
        self.assertNotIn("lint-schema", result.stdout)


class Supply(unittest.TestCase):
    def test_supply_runs_deny_audit_and_gitleaks(self):
        steps = gate_runner.phase_supply(NONE, [], which=lambda name: "/usr/bin/gitleaks")
        self.assertEqual([gate_runner.render_step(s) for s in steps],
                         ["just deny", "just audit", "bash scripts/ci-gitleaks.sh"])

    def test_absent_gitleaks_is_skipped_with_a_notice(self):
        steps = gate_runner.phase_supply(NONE, [], which=lambda name: None)
        self.assertTrue(steps[-1]["skip"])
        self.assertTrue(gate_runner.render_step(steps[-1]).startswith("# gitleaks skipped: no gitleaks binary"))
        out = io.StringIO()
        with redirect_stdout(out):
            results = gate_runner.execute([("supply", [steps[-1]])])
        self.assertIn("skipped: no gitleaks binary", out.getvalue())
        self.assertEqual(results[0]["result"], "skip")
        self.assertEqual(results[0]["steps"][0]["result"], "skip")
        self.assertIn("SKIP  gitleaks", gate_runner.summary(results))

    def test_skipped_step_does_not_hide_a_failure(self):
        phases = [("supply", [gate_runner.step("deny", ["false"]),
                              gate_runner.step("gitleaks", ["true"], skip="absent")])]
        with redirect_stdout(io.StringIO()):
            results = gate_runner.execute(phases)
        self.assertEqual(results[0]["result"], "fail")

    def test_supply_chain_workflow_uses_the_same_scan(self):
        workflow = (ROOT / ".github" / "workflows" / "supply-chain.yml").read_text()
        self.assertIn("bash scripts/ci-gitleaks.sh", workflow)
        self.assertIn("run: just deny", workflow)
        self.assertIn("run: just audit", workflow)


class PhaseSelection(unittest.TestCase):
    def test_only_and_skip(self):
        self.assertEqual(gate_runner.choose_phases("fmt,gates", None), ["fmt", "gates"])
        self.assertEqual(gate_runner.choose_phases(None, "tests,doc"), ["fmt", "clippy", "gates", "supply", "scripts"])
        self.assertEqual(gate_runner.choose_phases("gates,fmt", "fmt"), ["gates"])

    def test_unknown_phase_is_refused(self):
        with self.assertRaises(gate_runner.GateError):
            gate_runner.choose_phases("lint", None)

    def test_plan_keeps_run_order(self):
        self.assertEqual([n for n, _ in gate_runner.plan(AFFECTED, [], ["tests", "fmt"], db_url=URL)],
                         ["fmt", "tests"])

    def test_database_is_prepared_once_and_only_when_needed(self):
        for phases in (["clippy"], ["doc"], ["clippy", "doc", "tests"]):
            with self.subTest(phases=phases):
                names = [n for n, _ in gate_runner.plan(AFFECTED, [], phases, db_url=URL)]
                self.assertEqual(names.count("db"), 1)
                self.assertEqual(names[0], "db")
        self.assertNotIn("db", [n for n, _ in gate_runner.plan(AFFECTED, [], ["tests"], db_url=URL)])

    def test_every_shard_starts_from_a_fresh_database(self):
        plan = commands(gate_runner.plan(dict(AFFECTED, shard_groups=["a", "b"]), [], ["tests"], db_url=URL))
        self.assertEqual(plan["tests"], shard("a") + shard("b"))
        for phases in (["fmt", "gates", "scripts"],):
            self.assertNotIn("db", [n for n, _ in gate_runner.plan(AFFECTED, [], phases)])
        root_only = dict(AFFECTED, test_packages=[], test_dirs=[], shard_groups=[])
        self.assertNotIn("db", [n for n, _ in gate_runner.plan(root_only, [], ["clippy", "doc", "tests"])])

    def test_a_test_workspace_plan_without_a_url_is_refused(self):
        with self.assertRaises(gate_runner.GateError):
            gate_runner.plan(AFFECTED, [], ["tests"])


class DryRun(unittest.TestCase):
    def test_unit_dry_run_prints_the_plan(self):
        result = dry_run(AFFECTED, "--unit", "systemprompt-files", "--only", "fmt,tests", env={"TEST_DATABASE_URL": URL})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines()[1:], ["  unit systemprompt-files", "[fmt]", "  just format-check",
                                                         "[tests]", *(f"  {c}" for c in shard("domain"))])

    def test_changed_dry_run_reports_the_base(self):
        head = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
        result = dry_run(AFFECTED, "--changed", "--base", "HEAD", "--skip", "tests,doc")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"  base {head}", result.stdout)
        self.assertNotIn("[tests]", result.stdout)
        self.assertIn("[clippy]", result.stdout)

    def test_force_full_selection_falls_back_to_the_full_plan(self):
        result = dry_run(FORCED, "--changed", "--base", "HEAD", "--only", "doc,tests", env={"TEST_DATABASE_URL": URL})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("full because Cargo.lock: matches force-full pattern **/Cargo.lock", result.stdout)
        self.assertIn(f"  {LIVE} just doc-check", result.stdout)
        self.assertIn("bash scripts/test-shard.sh shared\n", result.stdout)
        self.assertIn("bash scripts/test-shard.sh domain\n", result.stdout)
        self.assertIn(f"[db]\n  {PREP}", result.stdout)
        self.assertIn(f"{LIVE} bash scripts/test-db.sh\n  {LIVE} bash scripts/test-shard.sh shared", result.stdout)

    def test_full_dry_run_lists_every_source_gate(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = gate_runner.main(["--full", "--only", "gates", "--dry-run"], environ={})
        self.assertEqual(code, 0)
        lines = out.getvalue().splitlines()
        self.assertEqual(lines[2:], [f"  just {g}" for g in gate_scope.gate_list()]
                         + ["  cargo test --locked --manifest-path scripts/rust-contracts/Cargo.toml"])

    def test_dry_run_writes_no_log(self):
        with tempfile.TemporaryDirectory() as scratch:
            log = Path(scratch) / "runs.jsonl"
            with redirect_stdout(io.StringIO()):
                gate_runner.main(["--full", "--only", "fmt", "--dry-run"], environ={"GATE_LOG": str(log)})
            self.assertFalse(log.exists())

    def test_usage_errors(self):
        result = subprocess.run(["bash", str(ROOT / "scripts" / "gate.sh"), "--unit", "x", "--base", "HEAD"],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        result = subprocess.run(["bash", str(ROOT / "scripts" / "gate.sh"), "--full", "--only", "nope", "--dry-run"],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown phase", result.stderr)


class TestDatabase(unittest.TestCase):
    def url(self, env):
        return subprocess.run(["bash", str(ROOT / "scripts" / "test-db.sh"), "--print-url"], capture_output=True,
                              text=True, env={k: v for k, v in os.environ.items() if k != "TEST_DATABASE_URL"} | env)

    def test_print_url_honours_the_override_and_defaults(self):
        self.assertEqual(self.url({"TEST_DATABASE_URL": URL}).stdout.strip(), URL)
        self.assertTrue(self.url({}).stdout.strip().endswith("/systemprompt_test"))

    def test_unknown_argument_is_refused(self):
        result = subprocess.run(["bash", str(ROOT / "scripts" / "test-db.sh"), "--nope"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)

    def test_recipes_share_the_preparation(self):
        text = (ROOT / "justfile").read_text()
        self.assertEqual(text.count("bash scripts/test-db.sh\n"), 2)
        self.assertNotIn("systemprompt-test-migrate --message-format", text)


class GateLog(unittest.TestCase):
    def record(self, results):
        start = datetime(2026, 11, 13, 10, tzinfo=timezone.utc)
        return gate_runner.log_record(
            mode="unit", units=["systemprompt-files"], base=None, head="a" * 40, tree="b" * 40, selection=AFFECTED,
            results=results, started=start, finished=start + timedelta(seconds=42),
            environ={"CLAUDE_CODE_SESSION_ID": "s-1", "HOME": "/h"}, host="box")

    def test_record_shape(self):
        results = [{"phase": "fmt", "result": "pass", "seconds": 1.0, "steps": []},
                   {"phase": "tests", "result": "skip", "seconds": 0.0, "steps": []}]
        record = self.record(results)
        self.assertEqual(set(record), {"timestamp", "mode", "units", "base", "head", "tree", "selector_mode",
                                       "scope_size", "counts", "phases", "duration_seconds", "result", "host",
                                       "session"})
        self.assertEqual(record["timestamp"], "2026-11-13T10:00:00+00:00")
        self.assertEqual(record["duration_seconds"], 42.0)
        self.assertEqual(record["result"], "pass")
        self.assertEqual(record["session"], {"CLAUDE_CODE_SESSION_ID": "s-1"})
        self.assertEqual(record["counts"], {"clippy_packages": 2, "test_packages": 1, "affected_dirs": 2,
                                            "test_dirs": 1, "shard_groups": 1})
        self.assertEqual(record["phases"][0], {"phase": "fmt", "result": "pass", "seconds": 1.0})

    def test_any_failed_phase_fails_the_run(self):
        record = self.record([{"phase": "fmt", "result": "pass", "seconds": 1.0, "steps": []},
                              {"phase": "gates", "result": "fail", "seconds": 2.0, "steps": []}])
        self.assertEqual(record["result"], "fail")

    def test_append_log_writes_json_lines(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "nested" / "runs.jsonl"
            gate_runner.append_log(path, {"a": 1})
            gate_runner.append_log(path, {"a": 2})
            self.assertEqual([json.loads(l)["a"] for l in path.read_text().splitlines()], [1, 2])

    def test_execute_runs_every_step_and_records_failures(self):
        phases = [("gates", [gate_runner.step("bad", ["false"]), gate_runner.step("good", ["true"])]),
                  ("tests", [])]
        with redirect_stdout(io.StringIO()):
            results = gate_runner.execute(phases)
        self.assertEqual([o["result"] for o in results[0]["steps"]], ["fail", "pass"])
        self.assertEqual([p["result"] for p in results], ["fail", "skip"])
        self.assertIn("1 step(s) failed: gates: bad", gate_runner.summary(results))

    def test_tree_hash_is_a_tree_object(self):
        tree = gate_runner.tree_hash()
        kind = subprocess.run(["git", "-C", str(ROOT), "cat-file", "-t", tree], capture_output=True, text=True)
        self.assertEqual(kind.stdout.strip(), "tree")


if __name__ == "__main__":
    unittest.main()
