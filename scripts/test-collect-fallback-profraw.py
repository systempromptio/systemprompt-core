#!/usr/bin/env python3
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("collect-fallback-profraw.sh")
QUARANTINE = Path(__file__).with_name("quarantine-fallback-profraw.sh")


class CollectFallbackProfrawTests(unittest.TestCase):
    def test_collects_only_current_run_profiles_without_removing_sources(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "workspace with spaces"
            output = Path(raw) / "profiles"
            package = root / "crates" / "tests" / "integration" / "api package"
            target = root / "target"
            package.mkdir(parents=True)
            target.mkdir(parents=True)

            stale = package / "default_stale.profraw"
            stale.write_bytes(b"stale")
            marker = root / "run-marker"
            marker.write_bytes(b"")
            marker_time = marker.stat().st_mtime
            old_time = marker_time - 10
            os.utime(stale, (old_time, old_time))

            current = package / "default_current profile.profraw"
            current.write_bytes(b"current")
            ignored = target / "default_target.profraw"
            ignored.write_bytes(b"target")
            new_time = marker_time + 10
            os.utime(current, (new_time, new_time))
            os.utime(ignored, (new_time, new_time))

            result = subprocess.run(
                [SCRIPT, root, marker, output, target],
                check=True,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.stdout.strip(), "1")
            self.assertEqual(
                [path.read_bytes() for path in output.glob("*.profraw")],
                [b"current"],
            )
            self.assertEqual(stale.read_bytes(), b"stale")
            self.assertEqual(current.read_bytes(), b"current")
            self.assertEqual(ignored.read_bytes(), b"target")

    def test_missing_search_root_fails_instead_of_reporting_zero(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            marker = root / "run-marker"
            marker.write_bytes(b"")

            result = subprocess.run(
                [SCRIPT, root / "missing", marker, root / "profiles"],
                capture_output=True,
                text=True,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "profiles").exists())

    def test_quarantine_prevents_same_filename_from_merging_an_old_run(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "workspace"
            package = root / "crates" / "tests" / "integration" / "api"
            package.mkdir(parents=True)
            profile = package / "default_module_0_42.profraw"
            profile.write_bytes(b"old counters")
            archive = Path(raw) / "archive"

            quarantined = subprocess.run(
                [QUARANTINE, root, archive],
                check=True,
                capture_output=True,
                text=True,
            )
            self.assertEqual(quarantined.stdout.strip(), "1")
            archived = archive / profile.relative_to(root)
            self.assertEqual(archived.read_bytes(), b"old counters")
            self.assertFalse(profile.exists())

            marker = Path(raw) / "run-marker"
            marker.write_bytes(b"")
            profile.write_bytes(b"current counters")
            new_time = marker.stat().st_mtime + 10
            os.utime(profile, (new_time, new_time))
            output = Path(raw) / "profiles"
            collected = subprocess.run(
                [SCRIPT, root, marker, output],
                check=True,
                capture_output=True,
                text=True,
            )

            self.assertEqual(collected.stdout.strip(), "1")
            self.assertEqual((output / "fallback-1.profraw").read_bytes(), b"current counters")
            self.assertEqual(archived.read_bytes(), b"old counters")
            self.assertEqual(profile.read_bytes(), b"current counters")


if __name__ == "__main__":
    unittest.main()
