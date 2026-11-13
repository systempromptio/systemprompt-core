#!/usr/bin/env python3
"""scripts/ci-base.py finds the newest green ancestor, and gates in full whenever it cannot."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("ci_base", ROOT / "scripts" / "ci-base.py")
ci_base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci_base)

REPO = "owner/core"
HEAD, P1, P2, P3 = "h" * 40, "1" * 40, "2" * 40, "3" * 40


def run(sha, branch="future", **over):
    return {"head_sha": sha, "status": "completed", "conclusion": "success", "event": "push", "head_branch": branch,
            **over}


def fetcher(runs_by_workflow, calls=None):
    def fetch(path):
        if calls is not None:
            calls.append(path)
        for workflow, runs in runs_by_workflow.items():
            if f"/workflows/{workflow}/runs" in path:
                if isinstance(runs, Exception):
                    raise runs
                return {"workflow_runs": runs if path.endswith("&page=1") else []}
        raise AssertionError(path)
    return fetch


def everywhere(runs):
    return {workflow: runs for workflow in ci_base.GATE_WORKFLOWS}


def git(history, shallow="false", code=0):
    def call(*args):
        if args[0] == "rev-list":
            return subprocess.CompletedProcess(args, code, " ".join(history) + "\n", "bad revision")
        return subprocess.CompletedProcess(args, 0, shallow + "\n", "")
    return call


def args(*extra):
    return ci_base.parse_args(["--event", "push", "--ref", "refs/heads/future", "--sha", HEAD, "--before", P1,
                               "--repo", REPO, *extra])


class Resolve(unittest.TestCase):
    def resolve(self, arguments, runs, history=(HEAD, P1, P2, P3), **git_kw):
        return ci_base.resolve(arguments, fetch=fetcher(runs), git=git(list(history), **git_kw))

    def test_nearest_green_ancestor_wins(self):
        base, reason = self.resolve(args(), everywhere([run(P2), run(P3)]))
        self.assertEqual(base, P2)
        self.assertIn("future", reason)

    def test_the_commit_itself_is_never_its_own_base(self):
        base, _ = self.resolve(args(), everywhere([run(HEAD), run(P3)]))
        self.assertEqual(base, P3)

    def test_every_gate_workflow_must_be_green(self):
        runs = everywhere([run(P1), run(P3)])
        runs["supply-chain.yml"] = [run(P3)]
        base, _ = self.resolve(args(), runs)
        self.assertEqual(base, P3)

    def test_runs_of_other_branches_events_or_outcomes_do_not_count(self):
        for bad in (run(P1, branch="next"), run(P1, event="pull_request"), run(P1, conclusion="failure"),
                    run(P1, status="in_progress")):
            with self.subTest(bad=bad):
                base, _ = self.resolve(args(), everywhere([bad]))
                self.assertEqual(base, "")

    def test_no_green_ancestor_is_full(self):
        base, reason = self.resolve(args(), everywhere([run("9" * 40)]))
        self.assertEqual(base, "")
        self.assertIn("no ancestor", reason)

    def test_force_push_and_new_branch_are_full(self):
        for extra, words in ((["--forced", "true"], "force-push"), (["--before", ci_base.ZERO], "creates the branch"),
                             (["--before", ""], "creates the branch")):
            with self.subTest(extra=extra):
                base, reason = self.resolve(args(*extra), everywhere([run(P1)]))
                self.assertEqual(base, "")
                self.assertIn(words, reason)

    def test_other_events_are_full(self):
        for event in ("pull_request", "workflow_dispatch", "schedule"):
            arguments = ci_base.parse_args(["--event", event, "--sha", HEAD, "--repo", REPO])
            base, reason = self.resolve(arguments, everywhere([run(P1)]))
            self.assertEqual(base, "")
            self.assertIn(event, reason)

    def test_merge_group_searches_the_target_branch(self):
        calls = []
        arguments = ci_base.parse_args(["--event", "merge_group", "--base-ref", "refs/heads/next", "--sha", HEAD,
                                        "--repo", REPO])
        base, _ = ci_base.resolve(arguments, fetch=fetcher(everywhere([run(P1, branch="next")]), calls),
                                  git=git([HEAD, P1]))
        self.assertEqual(base, P1)
        self.assertTrue(all("branch=next&event=push&status=success" in call for call in calls))

    def test_api_failure_is_full(self):
        runs = everywhere([run(P1)])
        runs["quality.yml"] = ci_base.gh_api.GhApiError("HTTP 502")
        base, reason = self.resolve(args(), runs)
        self.assertEqual(base, "")
        self.assertIn("HTTP 502", reason)

    def test_shallow_or_unreadable_history_is_full(self):
        base, reason = self.resolve(args(), everywhere([run(P1)]), shallow="true")
        self.assertEqual((base, "shallow" in reason), ("", True))
        base, reason = self.resolve(args(), everywhere([run(P1)]), code=128)
        self.assertEqual((base, "rev-list" in reason), ("", True))

    def test_run_list_is_bounded(self):
        calls = []
        full_page = [run("9" * 40)] * ci_base.gh_api.PER_PAGE

        def fetch(path):
            calls.append(path)
            return {"workflow_runs": full_page}
        ci_base.green_shas(REPO, "future", 150, fetch=fetch)
        self.assertEqual(len(calls), 2 * len(ci_base.GATE_WORKFLOWS))


class Main(unittest.TestCase):
    def test_output_lines(self):
        with tempfile.NamedTemporaryFile("r", suffix=".out") as out:
            code = ci_base.main(["--event", "pull_request", "--sha", HEAD, "--repo", REPO, "--output", out.name])
            self.assertEqual(code, 0)
            lines = Path(out.name).read_text().splitlines()
        self.assertEqual(lines[0], "base=")
        self.assertTrue(lines[1].startswith("base_reason=pull_request"))


if __name__ == "__main__":
    unittest.main()
