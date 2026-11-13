#!/usr/bin/env python3
"""The shared `gh api` helper retries rate limits only and walks every page."""
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import gh_api  # noqa: E402


def done(code, out="", err=""):
    return subprocess.CompletedProcess([], code, out, err)


class Get(unittest.TestCase):
    def test_waits_for_rate_limit_reset(self):
        calls = [done(1, err="API rate limit exceeded"), done(0, '{"resources": {"core": {"reset": 100}}}'),
                 done(0, '{"ok": true}')]
        slept = []
        with patch.object(gh_api.subprocess, "run", side_effect=calls):
            self.assertEqual(gh_api.get("x", sleep=slept.append, now=lambda: 90), {"ok": True})
        self.assertEqual(slept, [11])

    def test_wait_is_bounded(self):
        calls = [done(1, err="rate limit"), done(0, '{"resources": {"core": {"reset": 0}}}'), done(0, "{}")]
        slept = []
        with patch.object(gh_api.subprocess, "run", side_effect=calls):
            gh_api.get("x", sleep=slept.append, now=lambda: 50)
        self.assertEqual(slept, [1])

    def test_other_errors_raise_at_once(self):
        with patch.object(gh_api.subprocess, "run", return_value=done(1, err="Not Found")) as run:
            with self.assertRaises(gh_api.GhApiError):
                gh_api.get("x")
        self.assertEqual(run.call_count, 1)

    def test_retries_are_bounded(self):
        calls = [done(1, err="rate limit"), done(0, '{"resources": {"core": {"reset": 0}}}')] + [done(1, err="rate limit")]
        with patch.object(gh_api.subprocess, "run", side_effect=calls):
            with self.assertRaises(gh_api.GhApiError):
                gh_api.get("x", attempts=2, sleep=lambda _: None, now=lambda: 0)


class Paginate(unittest.TestCase):
    def test_walks_until_a_short_page(self):
        bodies = {1: {"items": list(range(100))}, 2: {"items": [100]}}
        seen = []

        def fetch(path):
            seen.append(path)
            return bodies[int(path.rsplit("page=", 1)[1])]
        self.assertEqual(len(list(gh_api.paginate("a?b=1", "items", fetch=fetch))), 101)
        self.assertEqual(seen, ["a?b=1&per_page=100&page=1", "a?b=1&per_page=100&page=2"])

    def test_bare_list_bodies(self):
        self.assertEqual(list(gh_api.paginate("a", fetch=lambda path: [1, 2])), [1, 2])


if __name__ == "__main__":
    unittest.main()
