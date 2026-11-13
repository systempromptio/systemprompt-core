#!/usr/bin/env python3
"""scripts/ci-plan.py plans, aggregates and reports escapes from one selection,
and its job table agrees with the gate workflows.

`WorkflowParity` is also run on its own by quality.yml's "Local and CI gate
lists match" step.
"""
import importlib.util
import io
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("ci_plan", ROOT / "scripts" / "ci-plan.py")
ci_plan = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci_plan)
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
            test_packages=[], test_dirs=[], shard_groups=[], archive_sets=[], scope_size=0)
SHARD_MAP = {"groups": [("shared", ["/shared/"]), ("domain", ["/domain/"]), ("bridge", ["/bridge/"])],
             "archive_of": {"shared": "unit", "domain": "unit", "bridge": "bridge"}}
FUTURE, NEXT = "refs/heads/future", "refs/heads/next"


def needs(workflow, result="success", **override):
    out = {job: {"result": result, "outputs": {}} for job in ci_plan.JOBS[workflow]}
    for job, value in override.items():
        out[job.replace("_", "-")] = {"result": value, "outputs": {}}
    return out


def affected_policy():
    return patch.object(ci_plan, "AFFECTED_BRANCHES", frozenset({"future"}))


class Policy(unittest.TestCase):
    def test_every_branch_and_event_is_full_today(self):
        for event in ("push", "pull_request", "merge_group", "workflow_dispatch", "schedule"):
            for ref in (FUTURE, NEXT, "refs/heads/main", "refs/pull/1/merge"):
                self.assertEqual(ci_plan.policy(event, ref), "full", (event, ref))

    def test_shadow_is_future_pushes_only(self):
        self.assertTrue(ci_plan.is_shadow("push", FUTURE))
        for event, ref in (("push", NEXT), ("pull_request", FUTURE), ("workflow_dispatch", FUTURE),
                           ("merge_group", FUTURE)):
            self.assertFalse(ci_plan.is_shadow(event, ref), (event, ref))


class Decisions(unittest.TestCase):
    def test_affected_selection(self):
        runs = ci_plan.decisions("quality", AFFECTED, "affected")
        self.assertTrue(runs["lint"] and runs["docs"] and runs["lint-tests"] and runs["msrv"])
        self.assertFalse(runs["bridge-web"] or runs["bridge-native"])
        self.assertTrue(runs["source-gates"] and runs["file-size"])

    def test_none_selection_keeps_only_unconditional_jobs(self):
        runs = ci_plan.decisions("ci", NONE, "affected")
        self.assertEqual({job for job, run in runs.items() if run}, {"affected", "gate-plan", "fmt"})

    def test_full_plan_runs_everything(self):
        for workflow in ci_plan.JOBS:
            self.assertTrue(all(ci_plan.decisions(workflow, NONE, "full").values()), workflow)
            self.assertTrue(all(ci_plan.decisions(workflow, dict(NONE, mode="full"), "affected").values()))

    def test_unknown_mode_is_refused(self):
        with self.assertRaises(ci_plan.PlanError):
            ci_plan.decisions("ci", dict(NONE, mode="partial"), "affected")


class Outputs(unittest.TestCase):
    def outputs(self, selection, event="push", ref=FUTURE, base="b" * 40, workflow="quality"):
        return ci_plan.plan_outputs(workflow, selection, event, ref, base, SHARD_MAP)

    def test_shadow_push_executes_full_and_carries_the_scoped_commands(self):
        out = self.outputs(AFFECTED)
        self.assertEqual(out["plan_mode"], "full")
        self.assertTrue(out["shadow"])
        self.assertEqual(out["clippy_args"], "--workspace")
        self.assertEqual(out["gate_args"], "--full")
        self.assertEqual(out["shadow_clippy_args"], "-p systemprompt-api -p systemprompt-files")
        self.assertEqual(out["shadow_gate_args"], f"--changed --base {'b' * 40} --selection target/ci/selection.json")
        self.assertFalse(out["shadow_bridge"])
        self.assertTrue(out["bridge"])
        self.assertTrue(all(out[key] for key in out if key.startswith("run_")))

    def test_full_matrices_come_from_test_shard(self):
        out = self.outputs(AFFECTED, workflow="ci", ref=NEXT)
        self.assertEqual(json.loads(out["shard_groups"]), ["shared", "domain", "bridge"])
        self.assertEqual(json.loads(out["archive_sets"]), ["bridge", "unit"])
        self.assertFalse(out["shadow"])

    def test_affected_policy_follows_the_selection(self):
        with affected_policy():
            out = self.outputs(AFFECTED, workflow="ci")
            quality = self.outputs(AFFECTED)
        self.assertEqual(out["plan_mode"], "affected")
        self.assertEqual(json.loads(out["shard_groups"]), ["domain"])
        self.assertEqual(json.loads(out["archive_sets"]), ["unit"])
        self.assertEqual(quality["clippy_args"], "-p systemprompt-api -p systemprompt-files")
        self.assertEqual(quality["tests_args"], "-p systemprompt-files-tests")
        self.assertEqual(quality["gate_args"], f"--changed --base {'b' * 40} --selection target/ci/selection.json")
        self.assertFalse(quality["run_bridge_web"])
        self.assertTrue(quality["run_lint"])

    def test_affected_policy_with_a_full_selection_is_full(self):
        with affected_policy():
            out = self.outputs(dict(AFFECTED, mode="full"), workflow="ci")
        self.assertEqual(out["plan_mode"], "full")
        self.assertEqual(len(json.loads(out["shard_groups"])), 3)

    def test_no_base_gates_in_full(self):
        self.assertEqual(ci_plan.gate_args(AFFECTED, "", False), "--full")

    def test_bad_package_names_are_refused(self):
        with self.assertRaises(ci_plan.PlanError):
            ci_plan.package_args(["ok", "bad name; rm"], False)

    def test_render_outputs(self):
        text = ci_plan.render_outputs({"a": True, "b": False, "c": "x y"})
        self.assertEqual(text, "a=true\nb=false\nc=x y\n")
        with self.assertRaises(ci_plan.PlanError):
            ci_plan.render_outputs({"a": "x\ny"})

    def test_plan_without_base_writes_a_full_selection(self):
        with tempfile.TemporaryDirectory() as scratch:
            selection = Path(scratch) / "selection.json"
            output = Path(scratch) / "out"
            with redirect_stdout(io.StringIO()):
                code = ci_plan.main(["plan", "--workflow", "ci", "--event", "push", "--ref", FUTURE,
                                     "--selection", str(selection), "--base-reason", "force-push",
                                     "--output", str(output)])
            self.assertEqual(code, 0)
            written = json.loads(selection.read_text())
            self.assertEqual(written["mode"], "full")
            self.assertEqual(written["reasons"], ["force-push"])
            lines = dict(line.split("=", 1) for line in output.read_text().splitlines())
        self.assertEqual(lines["plan_mode"], "full")
        self.assertEqual(lines["shadow_gate_args"], "--full")
        self.assertEqual(json.loads(lines["shard_groups"]), written["shard_groups"])


class Aggregate(unittest.TestCase):
    def verdict(self, workflow, need, selection=AFFECTED, event="push", ref=FUTURE):
        return ci_plan.aggregate(workflow, need, selection, event, ref)[0]

    def test_all_green_passes(self):
        for workflow in ci_plan.JOBS:
            self.assertTrue(self.verdict(workflow, needs(workflow)), workflow)

    def test_skip_the_selection_explains_passes_under_the_affected_policy(self):
        with affected_policy():
            self.assertTrue(self.verdict("quality", needs("quality", bridge_web="skipped", bridge_native="skipped")))
            self.assertTrue(self.verdict("ci", needs("ci", build="skipped", migrator="skipped", test="skipped",
                                                    test_tools="skipped", build_no_engine="skipped",
                                                    schema_ladder="skipped"), selection=NONE))

    def test_skip_the_selection_does_not_explain_fails(self):
        with affected_policy():
            self.assertFalse(self.verdict("quality", needs("quality", lint="skipped")))
            self.assertFalse(self.verdict("ci", needs("ci", fmt="skipped"), selection=NONE))

    def test_any_skip_fails_under_the_full_policy(self):
        self.assertFalse(self.verdict("quality", needs("quality", bridge_web="skipped")))
        self.assertFalse(self.verdict("ci", needs("ci", test="skipped"), selection=NONE, ref=NEXT))

    def test_failure_or_cancellation_fails(self):
        with affected_policy():
            for result in ("failure", "cancelled"):
                self.assertFalse(self.verdict("ci", needs("ci", test=result)), result)
                self.assertFalse(self.verdict("ci", needs("ci", build_no_engine=result), selection=NONE), result)

    def test_failed_selection_job_fails_everything(self):
        with affected_policy():
            self.assertFalse(self.verdict("ci", needs("ci", affected="failure")))
            self.assertFalse(self.verdict("quality", needs("quality"), selection=None))

    def test_full_selection_needs_every_job(self):
        with affected_policy():
            self.assertFalse(self.verdict("ci", needs("ci", test="skipped"), selection=dict(NONE, mode="full")))

    def test_needs_must_match_the_table(self):
        extra = dict(needs("ci"), stray={"result": "success"})
        self.assertFalse(self.verdict("ci", extra))
        missing = needs("ci")
        del missing["test"]
        self.assertFalse(self.verdict("ci", missing))

    def test_supply_chain_needs_no_selection(self):
        self.assertTrue(self.verdict("supply-chain", needs("supply-chain"), selection=None))
        self.assertFalse(self.verdict("supply-chain", needs("supply-chain", audit="skipped"), selection=None))

    def test_main_reads_the_environment(self):
        environ = {"NEEDS": json.dumps(needs("ci", test="cancelled")), "SELECTION": json.dumps(AFFECTED)}
        with redirect_stdout(io.StringIO()) as out:
            code = ci_plan.main(["aggregate", "--workflow", "ci", "--event", "push", "--ref", FUTURE], environ)
        self.assertEqual(code, 1)
        self.assertIn("test: cancelled", out.getvalue())
        environ = {"NEEDS": "not json"}
        with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            self.assertEqual(ci_plan.main(["aggregate", "--workflow", "ci", "--event", "push"], environ), 2)


class Escapes(unittest.TestCase):
    def test_entry_names(self):
        self.assertEqual(ci_plan.entry_of("ci", "Test (domain)"), ("test", "domain"))
        self.assertEqual(ci_plan.entry_of("ci", "Test tools (unit)"), ("test-tools", "unit"))
        self.assertEqual(ci_plan.entry_of("ci", "Schema ladder / Upgrade (v1)"), ("schema-ladder", "Upgrade (v1)"))
        self.assertEqual(ci_plan.entry_of("quality", "Source gates: bridge"), ("source-gates", "bridge"))
        self.assertEqual(ci_plan.entry_of("quality", "MSRV check (Rust 1.96)"), ("msrv", "1.96"))
        self.assertIsNone(ci_plan.entry_of("quality", "Escape report"))

    def test_skipped_entries_and_jobs(self):
        jobs = [{"name": "Test (shared)", "conclusion": "failure"}, {"name": "Test (domain)", "conclusion": "failure"},
                {"name": "Format", "conclusion": "failure"}]
        found = ci_plan.escapes("ci", AFFECTED, jobs)
        self.assertEqual(found, [{"job": "Test (shared)", "kind": "skipped"}])
        found = ci_plan.escapes("quality", AFFECTED, [{"name": "Bridge web regression tests", "conclusion": "failure"}])
        self.assertEqual(found, [{"job": "Bridge web regression tests", "kind": "skipped"}])

    def test_scope_escapes_need_a_green_shadow(self):
        jobs = [{"name": "Clippy lint baseline", "conclusion": "failure"},
                {"name": ci_plan.SHADOW_CLIPPY, "conclusion": "success"},
                {"name": "Source gates: bridge", "conclusion": "failure"},
                {"name": ci_plan.SHADOW_GATES, "conclusion": "failure"},
                {"name": "MSRV check (Rust 1.96)", "conclusion": "failure"}]
        found = ci_plan.escapes("quality", AFFECTED, jobs)
        self.assertEqual(found, [{"job": "Clippy lint baseline", "kind": "scope", "shadow": ci_plan.SHADOW_CLIPPY},
                                 {"job": "MSRV check (Rust 1.96)", "kind": "unverified"}])

    def test_full_selection_has_no_escapes(self):
        jobs = [{"name": "Test (shared)", "conclusion": "failure"}]
        self.assertEqual(ci_plan.escapes("ci", dict(NONE, mode="full"), jobs), [])

    def test_summary_and_files(self):
        with tempfile.TemporaryDirectory() as scratch:
            selection, jobs = Path(scratch) / "s.json", Path(scratch) / "j.json"
            summary, out = Path(scratch) / "summary.md", Path(scratch) / "escapes.json"
            selection.write_text(json.dumps(NONE))
            jobs.write_text(json.dumps([{"name": "Build", "conclusion": "failure"}]))
            with redirect_stdout(io.StringIO()):
                code = ci_plan.main(["escapes", "--workflow", "ci", "--selection", str(selection), "--jobs", str(jobs),
                                     "--summary", str(summary), "--out", str(out)])
            self.assertEqual(code, 0)
            self.assertIn("| Build | skipped |", summary.read_text())
            self.assertEqual(json.loads(out.read_text())["escapes"], [{"job": "Build", "kind": "skipped"}])
        self.assertIn("No escapes", ci_plan.escape_summary("ci", NONE, []))


def workflow_jobs(name):
    """`{job id: {"name", "needs", "if", "text"}}` from a workflow file, without a YAML parser."""
    text = (WORKFLOWS / name).read_text()
    body = text[text.index("\njobs:\n") + 7:]
    blocks = re.split(r"(?m)^  ([a-z0-9-]+):\n", body)[1:]
    jobs = {}
    for job, block in zip(blocks[::2], blocks[1::2]):
        field = {key: (re.search(rf"(?m)^    {key}: (.*)$", block) or [None, None])[1] for key in ("name", "needs", "if")}
        jobs[job] = dict(field, text=block)
    return jobs


def needs_list(value):
    return sorted(part.strip() for part in value.strip("[]").split(","))


class WorkflowParity(unittest.TestCase):
    FILES = {"ci": "ci.yml", "quality": "quality.yml", "supply-chain": "supply-chain.yml"}
    AGGREGATES = {"ci": "ci-passed", "quality": "quality-passed", "supply-chain": "supply-chain-passed"}

    def test_every_job_has_a_rule_and_the_aggregate_needs_exactly_the_ruled_jobs(self):
        for workflow, file in self.FILES.items():
            jobs = workflow_jobs(file)
            aggregate = self.AGGREGATES[workflow]
            expected = set(ci_plan.JOBS[workflow]) | set(ci_plan.SHADOW_JOBS[workflow]) | {aggregate}
            if workflow != "supply-chain":
                expected |= set(ci_plan.REPORT_JOBS)
            self.assertEqual(set(jobs), expected, workflow)
            self.assertEqual(needs_list(jobs[aggregate]["needs"]), sorted(ci_plan.JOBS[workflow]), workflow)
            self.assertEqual(jobs[aggregate]["if"], "always()", workflow)
            self.assertIn(f"ci-plan.py aggregate --workflow {workflow} ", jobs[aggregate]["text"])

    def test_planned_jobs_read_their_decision(self):
        for workflow in ("ci", "quality"):
            jobs = workflow_jobs(self.FILES[workflow])
            outputs = jobs["affected"]["text"]
            for job in ci_plan.JOBS[workflow]:
                if job in ci_plan.PLAN_JOBS:
                    continue
                key = f"run_{job.replace('-', '_')}"
                self.assertEqual(jobs[job]["if"], f"needs.affected.outputs.{key} == 'true'", (workflow, job))
                self.assertIn(f"      {key}: ${{{{ steps.plan.outputs.{key} }}}}", outputs, (workflow, job))
                self.assertIn("affected", jobs[job]["needs"], (workflow, job))
            report = jobs["escapes"]
            self.assertEqual(needs_list(report["needs"]),
                             sorted(set(ci_plan.JOBS[workflow]) - {"gate-plan"} | set(ci_plan.SHADOW_JOBS[workflow])))
            self.assertIn(f"ci-plan.py escapes --workflow {workflow} ", report["text"])

    def test_display_names_match_the_table(self):
        for workflow, file in self.FILES.items():
            jobs = workflow_jobs(file)
            for job, rule in ci_plan.JOBS[workflow].items():
                if rule.name and "{}" not in rule.name:
                    self.assertEqual(jobs[job]["name"].strip('"'), rule.name, (workflow, job))
            for job, name in ci_plan.SHADOW_JOBS[workflow].items():
                self.assertEqual(jobs[job]["name"].strip('"'), name, (workflow, job))

    def test_source_gate_matrix_is_the_gate_table(self):
        block = workflow_jobs("quality.yml")["source-gates"]["text"]
        entries = re.findall(r'- group: (\S+)\n\s+title: "([^"]+)"', block)
        self.assertEqual(dict(entries), ci_plan.GATE_GROUP_NAMES)
        self.assertEqual(sorted(ci_plan.GATE_GROUP_NAMES), sorted(gate_runner.GROUPS))
        table = gate_scope.gate_table()
        self.assertEqual(sorted(table), sorted(gate_scope.gate_list()))
        self.assertEqual({row["group"] for row in table.values()}, set(gate_runner.GROUPS))
        self.assertIn('bash scripts/gate.sh $GATE_ARGS --only gates --group "$GROUP"', block)
        for gate in gate_scope.gate_list():
            self.assertNotRegex(block, rf"run: just {re.escape(gate)}$", gate)

    def test_fmt_runs_through_gate(self):
        self.assertIn("run: bash scripts/gate.sh --full --only fmt", workflow_jobs("ci.yml")["fmt"]["text"])

    def test_gate_workflows_accept_merge_groups_and_name_the_ruleset_contexts(self):
        for workflow, file in self.FILES.items():
            text = (WORKFLOWS / file).read_text()
            self.assertRegex(text, r"(?m)^  merge_group:$", file)
            name = workflow_jobs(file)[self.AGGREGATES[workflow]]["name"]
            self.assertIn("github.event_name == 'merge_group'", name, file)
            self.assertIn("github.event_name }}-${{ inputs.ref || github.ref }}", text, file)

    def test_nightly_dispatches_every_gate_workflow_on_both_branches(self):
        text = (WORKFLOWS / "nightly.yml").read_text()
        self.assertIn("branch: [next, future]", text)
        self.assertIn("workflow: [ci.yml, quality.yml, supply-chain.yml]", text)
        self.assertIn("cancel-in-progress: false", text)


if __name__ == "__main__":
    unittest.main()
