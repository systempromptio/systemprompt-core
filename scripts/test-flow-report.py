#!/usr/bin/env python3
"""Exercise every flow-report statistic with synthetic git history and Actions runs."""
from datetime import datetime, timedelta, timezone
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("flow", Path(__file__).with_name("flow-report.py"))
flow = importlib.util.module_from_spec(spec)
spec.loader.exec_module(flow)

CRATES = ["crates/shared/models", "crates/domain/files", "crates/tests/unit/x"]
KERNEL = ["crates/shared/", "crates/domain/users/"]


def commit(sha, stamp, *files):
    return {"sha": sha, "time": stamp, "files": list(files)}


def run(run_id, sha, workflow, conclusion="success", created="2026-11-01T10:00:00Z", started=None,
        updated="2026-11-01T10:20:00Z", attempt=1, status="completed"):
    return {"id": run_id, "name": workflow, "path": f".github/workflows/{workflow}", "head_sha": sha, "event": "push",
            "head_branch": "next", "status": status, "conclusion": conclusion, "run_attempt": attempt,
            "created_at": created, "run_started_at": started or created, "updated_at": updated}


def green(sha, base_id, end="2026-11-01T10:20:00Z"):
    return [run(base_id, sha, "ci.yml", updated=end), run(base_id + 1, sha, "quality.yml"),
            run(base_id + 2, sha, "supply-chain.yml", created="2026-11-01T10:01:00Z")]


class Statistics(unittest.TestCase):
    def test_percentile_is_nearest_rank(self):
        self.assertEqual(flow.percentile([5, 1, 3, 2, 4], 0.5), 3)
        self.assertEqual(flow.percentile(range(1, 11), 0.9), 9)
        self.assertEqual(flow.percentile([7], 0.9), 7)
        self.assertIsNone(flow.percentile([], 0.5))

    def test_summary_counts_values(self):
        self.assertEqual(flow.summary([1, 2, 3, 4]), {"n": 4, "median": 2, "p90": 4})


class GitSide(unittest.TestCase):
    def test_parse_git_log_splits_commits_and_files(self):
        text = "\x00aaa\t2026-11-01T10:00:00+00:00\n\nCHANGELOG.md\nCargo.lock\n\x00bbb\t2026-11-02T10:00:00+00:00\n\n"
        self.assertEqual(flow.parse_git_log(text), [
            commit("aaa", "2026-11-01T10:00:00+00:00", "CHANGELOG.md", "Cargo.lock"),
            commit("bbb", "2026-11-02T10:00:00+00:00")])

    def test_crate_dirs_from_manifests(self):
        self.assertEqual(flow.crate_dirs_from_tree(["Cargo.toml", "crates/a/Cargo.toml", "crates/a/src/lib.rs"]),
                         ["", "crates/a"])

    def test_owning_crate_is_longest_prefix_at_a_path_boundary(self):
        dirs = ["crates/shared/models", "crates/shared/models/derive", "crates/shared/model"]
        self.assertEqual(flow.owning_crate("crates/shared/models/derive/src/lib.rs", dirs), "crates/shared/models/derive")
        self.assertEqual(flow.owning_crate("crates/shared/models/src/lib.rs", dirs), "crates/shared/models")
        self.assertIsNone(flow.owning_crate("CHANGELOG.md", dirs))

    def test_top_files_orders_by_count_then_path(self):
        commits = [commit("a", "", "b.rs", "a.rs"), commit("b", "", "b.rs", "a.rs"), commit("c", "", "c.rs", "b.rs")]
        self.assertEqual(flow.top_files(commits, 2), [("b.rs", 3), ("a.rs", 2)])

    def test_blast_radius_counts_files_and_crates(self):
        commits = [commit("a", "", "crates/shared/models/src/a.rs", "crates/shared/models/src/b.rs"),
                   commit("b", "", "crates/shared/models/x.rs", "crates/domain/files/y.rs", "CHANGELOG.md"),
                   commit("c", "", "README.md")]
        radius = flow.blast_radius(commits, CRATES)
        self.assertEqual(radius["files_per_commit"], {"n": 3, "median": 2, "p90": 3})
        self.assertEqual(radius["crates_per_commit"], {"n": 3, "median": 1, "p90": 2})
        self.assertEqual(radius["crates_histogram"], {"0": 1, "1": 1, "2": 1})

    def test_blast_radius_buckets_ten_and_more(self):
        dirs = [f"c{i}" for i in range(12)]
        radius = flow.blast_radius([commit("a", "", *(f"c{i}/f" for i in range(12)))], dirs)
        self.assertEqual(radius["crates_histogram"], {"10+": 1})

    def test_parse_kernel_skips_blank_and_comment_lines(self):
        self.assertEqual(flow.parse_kernel("# kernel\ncrates/shared/\n\n crates/app/runtime/ \n"),
                         ["crates/shared/", "crates/app/runtime/"])

    def test_kernel_list_names_existing_crates(self):
        root = Path(__file__).resolve().parents[1]
        for prefix in flow.parse_kernel((root / "scripts" / "flow-report-kernel.txt").read_text()):
            self.assertTrue((root / prefix).is_dir(), prefix)

    def test_kernel_churn_groups_by_iso_week(self):
        commits = [commit("a", "2026-11-02T09:00:00+00:00", "crates/shared/models/a.rs"),
                   commit("b", "2026-11-03T09:00:00+00:00", "crates/domain/files/a.rs"),
                   commit("c", "2026-11-10T09:00:00+00:00", "crates/domain/users/a.rs", "README.md")]
        self.assertEqual(flow.kernel_churn(commits, KERNEL), [
            {"week": "2026-W45", "kernel": 1, "total": 2}, {"week": "2026-W46", "kernel": 1, "total": 1}])

    def test_release_lead_time_uses_first_containing_tag(self):
        commits = [commit("a", "2026-11-01T00:00:00+00:00"), commit("b", "2026-11-02T00:00:00+00:00"),
                   commit("c", "2026-11-03T00:00:00+00:00")]
        releases = [{"tag": "v1.0.0", "time": "2026-11-02T12:00:00+00:00", "shas": {"a", "b"}},
                    {"tag": "v1.0.1", "time": "2026-11-05T00:00:00+00:00", "shas": {"a", "b"}}]
        result = flow.release_lead_times(commits, releases)
        self.assertEqual(result["hours"], {"n": 2, "median": 12.0, "p90": 36.0})
        self.assertEqual(result["unreleased"], 1)


class ActionsSide(unittest.TestCase):
    def test_push_to_green_spans_first_creation_to_last_gate_end(self):
        runs = green("s", 1, end="2026-11-01T10:30:00Z")
        self.assertEqual(flow.push_to_green(flow.group_by_sha(runs)["s"]), 30.0)

    def test_push_to_green_uses_latest_run_per_gate(self):
        runs = green("s", 10) + [run(5, "s", "ci.yml", "failure", updated="2026-11-01T10:05:00Z")]
        self.assertEqual(flow.push_to_green(flow.group_by_sha(runs)["s"]), 20.0)
        runs = green("s", 1) + [run(9, "s", "ci.yml", "failure")]
        self.assertIsNone(flow.push_to_green(flow.group_by_sha(runs)["s"]))

    def test_push_to_green_requires_every_gate_completed(self):
        self.assertIsNone(flow.push_to_green(flow.group_by_sha(green("s", 1)[:2])["s"]))
        runs = green("s", 1)[:2] + [run(3, "s", "supply-chain.yml", None, status="in_progress")]
        self.assertIsNone(flow.push_to_green(flow.group_by_sha(runs)["s"]))

    def test_group_by_sha_ignores_non_gate_workflows(self):
        grouped = flow.group_by_sha([run(1, "s", "coverage.yml"), run(2, "s", "ci.yml@refs/heads/next")])
        self.assertEqual(set(grouped["s"]), {"ci.yml"})

    def test_lead_and_share(self):
        runs = green("a", 1) + green("b", 10) + [run(20, "c", "ci.yml", "failure", updated="2026-11-01T10:10:00Z")]
        result = flow.lead_and_share(runs)
        self.assertEqual((result["pushed_shas"], result["green_shas"]), (3, 2))
        self.assertEqual(result["push_to_green_minutes"], {"n": 2, "median": 20.0, "p90": 20.0})
        self.assertEqual(result["gate_minutes_per_sha"], {"n": 3, "median": 59.0, "p90": 59.0})

    def test_rework_counts_red_runs_reruns_and_conclusions(self):
        runs = (green("a", 1) + [run(5, "a", "ci.yml", "failure"), run(6, "b", "quality.yml", "failure"),
                                 run(7, "b", "quality.yml", "failure", attempt=2), run(8, "b", "coverage.yml", "cancelled")])
        result = flow.rework(runs)
        self.assertEqual(result["red_runs_per_sha"], {"n": 2, "median": 1, "p90": 2})
        self.assertEqual(result["shas_with_red"], 2)
        self.assertEqual(result["reruns"], 1)
        self.assertEqual(result["conclusions"]["quality.yml"], {"success": 1, "failure": 2})
        self.assertEqual(result["conclusions"]["coverage.yml"], {"cancelled": 1})

    def test_rework_reports_status_of_unfinished_runs(self):
        result = flow.rework([run(1, "a", "ci.yml", None, status="queued")])
        self.assertEqual(result["conclusions"]["ci.yml"], {"queued": 1})

    def test_waste_sums_cancelled_run_minutes(self):
        runs = [run(1, "a", "ci.yml", "cancelled", started="2026-11-01T10:05:00Z", updated="2026-11-01T10:15:00Z"),
                run(2, "b", "ci.yml", "cancelled", updated="2026-11-01T10:05:30Z"),
                run(3, "c", "semver.yml", "cancelled"), run(4, "d", "ci.yml", "failure")]
        self.assertEqual(flow.waste(runs), {"per_workflow": {"ci.yml": 15.5, "semver.yml": 20.0}, "total": 35.5})

    def test_workflow_table_rates_decided_runs_and_times_successes(self):
        runs = [run(1, "a", "ci.yml", updated="2026-11-01T10:10:00Z"), run(2, "b", "ci.yml"),
                run(3, "c", "ci.yml", "failure", updated="2026-11-01T11:00:00Z"), run(4, "d", "ci.yml", "cancelled")]
        self.assertEqual(flow.workflow_table(runs), [{
            "workflow": "ci.yml", "runs": 4, "decided": 3, "success": 2, "success_rate": 0.667,
            "median_minutes": 10.0, "p90_minutes": 20.0}])


class Fetching(unittest.TestCase):
    def test_slices_cover_window_on_a_fixed_grid(self):
        start = datetime(2026, 11, 1, 7, tzinfo=timezone.utc)
        step = timedelta(days=3)
        parts = list(flow.slices(start, start + timedelta(days=7), step))
        later = list(flow.slices(start + timedelta(hours=5), start + timedelta(days=7, hours=5), step))
        self.assertTrue(all(end - begin == step for begin, end in parts))
        self.assertTrue(all(parts[i][1] == parts[i + 1][0] for i in range(len(parts) - 1)))
        self.assertLessEqual(parts[0][0], start)
        self.assertGreaterEqual(parts[-1][1], start + timedelta(days=7))
        self.assertEqual(parts[0], later[0])

    def test_fetch_slice_paginates_and_slims(self):
        page1 = [dict(run(i, "s", "ci.yml"), extra="x") for i in range(100)]
        pages = [{"total_count": 101, "workflow_runs": page1}, {"total_count": 101, "workflow_runs": [run(100, "s", "ci.yml")]}]
        start = datetime(2026, 11, 1, tzinfo=timezone.utc)
        with patch.object(flow, "gh_json", side_effect=pages) as gh:
            runs = flow.fetch_slice("o/r", "next", start, start + timedelta(days=1))
        self.assertEqual(len(runs), 101)
        self.assertNotIn("extra", runs[0])
        self.assertIn("page=2", gh.call_args_list[1].args[0])

    def test_fetch_slice_splits_windows_over_the_search_cap(self):
        start = datetime(2026, 11, 1, tzinfo=timezone.utc)
        responses = [{"total_count": 1500, "workflow_runs": []},
                     {"total_count": 1, "workflow_runs": [run(1, "s", "ci.yml")]},
                     {"total_count": 1, "workflow_runs": [run(2, "s", "ci.yml")]}]
        with patch.object(flow, "gh_json", side_effect=responses) as gh:
            runs = flow.fetch_slice("o/r", "next", start, start + timedelta(days=2))
        self.assertEqual([item["id"] for item in runs], [1, 2])
        self.assertEqual(gh.call_count, 3)

    def test_fetch_runs_reuses_settled_slices_and_refetches_recent_ones(self):
        now = datetime(2026, 11, 13, tzinfo=timezone.utc)
        since = datetime(2026, 11, 6, tzinfo=timezone.utc)
        with tempfile.TemporaryDirectory() as tmp:
            cache = Path(tmp) / "cache.json"
            with patch.object(flow, "fetch_slice", side_effect=lambda repo, branch, s, e: [
                    run(int(s.timestamp()), "s", "ci.yml", created=s.strftime("%Y-%m-%dT%H:%M:%SZ"))]) as fetch:
                first = flow.fetch_runs("o/r", "next", since, now, cache, now)
                self.assertEqual(fetch.call_count, 3)
                second = flow.fetch_runs("o/r", "next", since, now, cache, now)
                self.assertEqual(fetch.call_count, 4)
            self.assertEqual(first, second)
            self.assertEqual(len(first), 3)
            self.assertEqual(len(json.loads(cache.read_text())), 3)

    def test_gh_json_waits_for_rate_limit_reset(self):
        limited = flow.subprocess.CompletedProcess([], 1, "", "API rate limit exceeded")
        ok = flow.subprocess.CompletedProcess([], 0, '{"ok": true}', "")
        with patch.object(flow.subprocess, "run", side_effect=[limited, ok]), \
                patch.object(flow.release_proof, "command", return_value='{"resources": {"core": {"reset": 0}}}'), \
                patch.object(flow.time, "sleep") as sleep:
            self.assertEqual(flow.gh_json("x"), {"ok": True})
        sleep.assert_called_once_with(1)

    def test_gh_json_raises_on_other_errors(self):
        failed = flow.subprocess.CompletedProcess([], 1, "", "Not Found")
        with patch.object(flow.subprocess, "run", return_value=failed):
            with self.assertRaises(RuntimeError):
                flow.gh_json("x")


class Rendering(unittest.TestCase):
    def test_render_includes_git_and_actions_sections(self):
        commits = [commit("a" * 40, "2026-11-02T09:00:00+00:00", "crates/shared/models/a.rs")]
        report = {"branch": "next", "since": "s", "until": "u", "commits": 1, "first_sha": "aaa", "last_sha": "aaa",
                  "git": {"top_files": flow.top_files(commits, 5), "blast_radius": flow.blast_radius(commits, CRATES),
                          "kernel_churn": flow.kernel_churn(commits, KERNEL),
                          "landed_to_released": flow.release_lead_times(commits, [])}}
        text = flow.render(report)
        self.assertIn("`crates/shared/models/a.rs` | 1", text)
        self.assertNotIn("push → green", text)
        runs = green("s", 1)
        report["actions"] = {"runs": 3, "lead": flow.lead_and_share(runs), "rework": flow.rework(runs),
                             "waste": flow.waste(runs), "workflows": flow.workflow_table(runs)}
        text = flow.render(report)
        self.assertIn("1 of 1 pushed SHAs went green", text)
        self.assertIn("| ci.yml | 1 | 1.000 |", text)


if __name__ == "__main__":
    unittest.main()
