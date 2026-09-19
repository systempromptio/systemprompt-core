#!/usr/bin/env python3
"""Exercise fail-closed promotion evidence with GitHub response fixtures."""
import copy
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("proof", Path(__file__).with_name("release-proof.py"))
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)
SHA = "a" * 40
REPO = "systempromptio/systemprompt-core"
DATE = "2026-09-19T00:00:00Z"
RUN = dict(id=1, head_sha=SHA, event="pull_request", head_branch="promote", head_repository={"full_name": REPO},
           pull_requests=[{"number": 42}], created_at=DATE, status="completed", conclusion="success", run_attempt=1)
PR = dict(number=42, head={"sha": SHA, "ref": "promote", "repo": {"full_name": REPO}},
          base={"ref": "main", "sha": "b" * 40}, merge_commit_sha="c" * 40, created_at=DATE)


class ProofTests(unittest.TestCase):
    def select(self, runs):
        return proof.validate_runs(runs, SHA, 42, REPO, DATE)

    def test_missing_proof_refuses(self):
        with self.assertRaises(RuntimeError):
            self.select([])

    def test_latest_failure_overrides_earlier_success(self):
        with self.assertRaises(RuntimeError):
            self.select([RUN, dict(RUN, id=2, conclusion="failure")])

    def test_wrong_identity_or_incomplete_runs_refuse(self):
        for field, value in [("head_sha", "d" * 40), ("event", "push"), ("head_branch", "next"),
                             ("head_repository", {"full_name": "other/repo"}), ("pull_requests", [{"number": 43}]),
                             ("created_at", "2026-09-18T00:00:00Z"), ("status", "in_progress"),
                             ("conclusion", "cancelled"), ("conclusion", "skipped")]:
            with self.subTest(field=field, value=value), self.assertRaises(RuntimeError):
                self.select([dict(RUN, **{field: value})])

    def test_merged_run_retains_identity_when_github_empties_pr_list(self):
        self.assertEqual(self.select([dict(RUN, pull_requests=[])])["id"], 1)

    def response(self, path):
        if path.startswith("git/commits/"):
            return {"tree": {"sha": "tree"}}
        if path.startswith("actions/workflows/"):
            return {"workflow_runs": [RUN]}
        if path.startswith("actions/runs/"):
            return {"jobs": [{"name": name, "conclusion": "success"} for name in proof.WORKFLOWS.values()]}
        if path == "pulls/42":
            return PR
        self.fail(path)

    def test_valid_promotion(self):
        with patch.object(proof, "api", side_effect=lambda repo, path: self.response(path)):
            proof.verify_pr(REPO, PR, SHA)

    def test_changed_merge_tree_refuses(self):
        def response(repo, path):
            if path == "git/commits/" + PR["merge_commit_sha"]:
                return {"tree": {"sha": "different"}}
            return self.response(path)
        with patch.object(proof, "api", side_effect=response), self.assertRaises(RuntimeError):
            proof.verify_pr(REPO, PR, SHA)

    def test_head_or_base_race_refuses(self):
        for side in ["head", "base"]:
            def response(repo, path):
                value = copy.deepcopy(self.response(path))
                if path == "pulls/42":
                    value[side]["sha"] = "d" * 40
                return value
            with self.subTest(side=side), patch.object(proof, "api", side_effect=response), self.assertRaises(RuntimeError):
                proof.verify_pr(REPO, PR, SHA)

    def test_rerun_or_new_run_during_proof_refuses(self):
        for changed in [dict(RUN, run_attempt=2), dict(RUN, id=2), dict(RUN, status="in_progress")]:
            calls = {}
            def response(repo, path):
                if path.startswith("actions/workflows/"):
                    calls[path] = calls.get(path, 0) + 1
                    if calls[path] > 1:
                        return {"workflow_runs": [changed]}
                return self.response(path)
            with self.subTest(changed=changed), patch.object(proof, "api", side_effect=response), self.assertRaises(RuntimeError):
                proof.verify_pr(REPO, PR, SHA)

    def test_missing_aggregate_refuses(self):
        def response(repo, path):
            return {"jobs": []} if path.startswith("actions/runs/") else self.response(path)
        with patch.object(proof, "api", side_effect=response), self.assertRaises(RuntimeError):
            proof.verify_pr(REPO, PR, SHA)


if __name__ == "__main__":
    unittest.main()
