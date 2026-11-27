#!/usr/bin/env python3
"""lint-phone-closure refuses exactly the edges that take the phone binding crate outside its slices.

Each test writes a cargo metadata document for a fixture graph and runs the
gate over it with `--metadata`.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "lint-phone-closure.py"

LAYERS = {
    "systemprompt-identifiers": "shared",
    "systemprompt-models": "shared",
    "systemprompt-client": "shared",
    "systemprompt-database": "infra",
    "systemprompt-device": "infra",
    "systemprompt-machine": "infra",
    "systemprompt-engine": "domain",
    "systemprompt-proxy": "domain",
    "systemprompt-hosts": "domain",
    "systemprompt-drive": "domain",
    "systemprompt-runner": "domain",
    "systemprompt-users": "domain",
    "systemprompt-harness": "app",
    "systemprompt-mobile": "app",
    "systemprompt-api": "entry",
}
OUTSIDE = ("machine", "engine", "proxy", "hosts", "drive", "runner")


def dep(name, optional=False, features=(), kind=None, default=True):
    return {
        "name": name, "rename": None, "optional": optional, "features": list(features),
        "kind": kind, "uses_default_features": default,
    }


def graph(edges=None, features=None, without=()):
    """The fixture workspace; `edges` gives each crate its dependencies, `features` its feature table."""
    edges = edges or {}
    features = features or {}
    packages = [
        {
            "name": name,
            "manifest_path": f"/fixture/crates/{layer}/{name}/Cargo.toml",
            "dependencies": [d if isinstance(d, dict) else dep(d) for d in edges.get(name, ())],
            "features": features.get(name, {}),
        }
        for name, layer in LAYERS.items() if name not in without
    ]
    packages.append({
        "name": "systemprompt", "manifest_path": "/fixture/systemprompt/Cargo.toml",
        "dependencies": [dep("systemprompt-api")], "features": {},
    })
    return packages


class PhoneClosure(unittest.TestCase):
    def check(self, packages, expected):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as out:
            out.write(json.dumps({"packages": packages}))
        self.addCleanup(os.unlink, out.name)
        result = subprocess.run(
            ["python3", str(SCRIPT), "--metadata", out.name],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )
        lines = sorted(line.split(":")[0].strip() for line in result.stdout.splitlines() if line.startswith("  "))
        self.assertEqual(lines, sorted(expected), result.stdout + result.stderr)
        self.assertEqual(result.returncode, 1 if expected else 0, result.stdout + result.stderr)

    def test_empty_binding_crate_passes(self):
        self.check(graph(), [])

    def test_device_client_and_shared_pass(self):
        self.check(graph({
            "systemprompt-mobile": ["systemprompt-device", "systemprompt-harness", "systemprompt-client"],
            "systemprompt-device": ["systemprompt-models"],
            "systemprompt-harness": ["systemprompt-device", "systemprompt-client"],
            "systemprompt-client": ["systemprompt-models", "systemprompt-identifiers"],
        }), [])

    def test_every_device_crate_outside_the_phone_slices_fails(self):
        for short in OUTSIDE:
            with self.subTest(name=short):
                target = f"systemprompt-{short}"
                self.check(graph({"systemprompt-mobile": [target]}), [f"systemprompt-mobile -> {target}"])

    def test_server_crates_and_the_facade_fail(self):
        targets = ["systemprompt-database", "systemprompt-users", "systemprompt-api", "systemprompt"]
        self.check(
            graph({"systemprompt-mobile": targets}),
            [f"systemprompt-mobile -> {target}" for target in targets],
        )

    def test_refused_crate_reached_through_a_slice_fails_at_that_edge(self):
        self.check(
            graph({
                "systemprompt-mobile": ["systemprompt-harness"],
                "systemprompt-harness": ["systemprompt-engine"],
            }),
            ["systemprompt-harness -> systemprompt-engine"],
        )

    def test_optional_slice_the_phone_does_not_name_is_outside_the_closure(self):
        self.check(
            graph(
                {
                    "systemprompt-mobile": [dep("systemprompt-harness", features=["client"], default=False)],
                    "systemprompt-harness": [dep("systemprompt-engine", optional=True), "systemprompt-client"],
                },
                {"systemprompt-harness": {"client": [], "engine": ["dep:systemprompt-engine"]}},
            ),
            [],
        )

    def test_optional_slice_a_named_feature_enables_fails(self):
        self.check(
            graph(
                {
                    "systemprompt-mobile": [dep("systemprompt-harness", features=["engine"], default=False)],
                    "systemprompt-harness": [dep("systemprompt-engine", optional=True)],
                },
                {"systemprompt-harness": {"engine": ["dep:systemprompt-engine"]}},
            ),
            ["systemprompt-harness -> systemprompt-engine"],
        )

    def test_default_features_of_a_slice_are_followed(self):
        self.check(
            graph(
                {
                    "systemprompt-mobile": ["systemprompt-harness"],
                    "systemprompt-harness": [dep("systemprompt-proxy", optional=True)],
                },
                {"systemprompt-harness": {"default": ["proxy"], "proxy": ["dep:systemprompt-proxy"]}},
            ),
            ["systemprompt-harness -> systemprompt-proxy"],
        )

    def test_every_feature_of_the_binding_crate_is_judged(self):
        self.check(
            graph(
                {"systemprompt-mobile": [dep("systemprompt-runner", optional=True)]},
                {"systemprompt-mobile": {"runs": ["dep:systemprompt-runner"]}},
            ),
            ["systemprompt-mobile -> systemprompt-runner"],
        )

    def test_feature_forwarded_to_a_slice_is_followed(self):
        self.check(
            graph(
                {
                    "systemprompt-mobile": [dep("systemprompt-harness", default=False)],
                    "systemprompt-harness": [dep("systemprompt-drive", optional=True)],
                },
                {
                    "systemprompt-mobile": {"documents": ["systemprompt-harness/drive"]},
                    "systemprompt-harness": {"drive": ["dep:systemprompt-drive"]},
                },
            ),
            ["systemprompt-harness -> systemprompt-drive"],
        )

    def test_dev_dependency_is_ignored(self):
        self.check(graph({"systemprompt-mobile": [dep("systemprompt-database", kind="dev")]}), [])

    def test_missing_binding_crate_fails(self):
        self.check(graph(without=("systemprompt-mobile",)), ["systemprompt-mobile"])


if __name__ == "__main__":
    unittest.main()
