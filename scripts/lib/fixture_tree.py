"""A temporary repository tree for the source-gate unit tests.

`GateFixture` writes files under a fresh directory and runs a gate script
with that directory as its ROOT, returning the `path:line` keys the gate
reported.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class GateFixture(unittest.TestCase):
    script = None

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)

    def write(self, rel, text):
        path = self.base / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def run_gate(self):
        return subprocess.run(
            ["python3", str(ROOT / "scripts" / self.script), str(self.base)],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def check(self, expected):
        result = self.run_gate()
        found = sorted(line.strip().split(": ", 1)[0] for line in result.stdout.splitlines() if line.startswith("  "))
        self.assertEqual(found, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)
