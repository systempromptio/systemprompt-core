#!/usr/bin/env python3
"""lint-platform-confinement reports every target-system condition of a device
crate that stands outside a platform module, grouped by crate, and nothing in
comments, strings, feature or test conditions, platform modules or other crates."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import unittest

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS / "lib"))
from fixture_tree import ROOT, GateFixture  # noqa: E402

SCRIPT = SCRIPTS / "lint-platform-confinement.py"
ENGINE = "crates/domain/engine"
DEVICE = "crates/infra/device"


def closure_gate_crates():
    spec = importlib.util.spec_from_file_location("lint_device_closure", SCRIPTS / "lint-device-closure.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.DEVICE


CRATES = closure_gate_crates()


def crate_dir(name):
    """Where the fixture puts a device crate; the two named above keep their real layer."""
    short = name.removeprefix("systemprompt-")
    return {"engine": ENGINE, "device": DEVICE}.get(short, f"crates/app/{short}")


class PlatformConfinement(GateFixture):
    script = "lint-platform-confinement.py"

    def setUp(self):
        super().setUp()
        for name in CRATES:
            self.write(f"{crate_dir(name)}/Cargo.toml", f'[package]\nname = "{name}"\nversion = "0.0.0"\n')
            self.write(f"{crate_dir(name)}/src/lib.rs", "pub fn f() {}\n")

    def run_gate(self, *flags):
        return subprocess.run(
            ["python3", str(SCRIPT), *flags, str(self.base)],
            cwd=ROOT, capture_output=True, text=True, env=os.environ | {"LC_ALL": "C"},
        )

    def test_the_skeleton_passes(self):
        result = self.run_gate()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"{len(CRATES)} files of {len(CRATES)} device crates", result.stdout)

    def test_every_form_of_condition_fails_in_every_device_crate(self):
        forms = (
            "#[cfg(windows)]\nfn f() {}\n",
            "#[cfg(unix)]\nfn f() {}\n",
            '#[cfg(target_os = "linux")]\nfn f() {}\n',
            '#[cfg(target_family = "unix")]\nfn f() {}\n',
            '#[cfg(target_vendor = "apple")]\nfn f() {}\n',
            '#[cfg(target_env = "musl")]\nfn f() {}\n',
            "#![cfg(windows)]\n",
            "#[cfg(not(windows))]\nfn f() {}\n",
            '#[cfg(all(test, any(feature = "a", target_os = "macos")))]\nfn f() {}\n',
            "#[cfg_attr(windows, allow(dead_code))]\nfn f() {}\n",
            '#[cfg_attr(target_os = "macos", path = "mac.rs")]\nmod m;\n',
            "#[cfg_attr(docsrs, doc(cfg(unix)))]\nfn f() {}\n",
            "#[doc(cfg(windows))]\nfn f() {}\n",
            "const W: bool = cfg!(windows);\n",
            'const W: bool = cfg! (target_os = "windows");\n',
            "#[cfg(windows)]\nmod platform;\n",
        )
        for name in CRATES:
            for form in forms:
                with self.subTest(name=name, form=form):
                    self.write(f"{crate_dir(name)}/src/a.rs", form)
                    self.check([f"{crate_dir(name)}/src/a.rs:1"])
            self.write(f"{crate_dir(name)}/src/a.rs", "pub fn f() {}\n")

    def test_conditions_inside_bodies_and_macros_fail(self):
        self.write(
            f"{ENGINE}/src/a.rs",
            "fn f() {\n    #[cfg(windows)]\n    g();\n    if cfg!(unix) {}\n}\n"
            "macro_rules! m { () => {\n    #[cfg(windows)]\n    fn h() {}\n} }\n"
            "struct S {\n    #[cfg(unix)]\n    mode: u32,\n}\n",
        )
        self.check([f"{ENGINE}/src/a.rs:{n}" for n in (2, 4, 7, 11)])

    def test_an_attribute_across_lines_is_one_finding_at_its_start(self):
        self.write(f"{ENGINE}/src/a.rs", 'pub fn f() {}\n#[cfg(any(\n    target_os = "linux",\n    windows,\n))]\nfn g() {}\n')
        result = self.run_gate()
        self.assertIn(f'  {ENGINE}/src/a.rs:2: target-system condition (target_os = "linux", windows)', result.stdout)
        self.check([f"{ENGINE}/src/a.rs:2"])

    def test_platform_modules_pass(self):
        body = '#[cfg(windows)]\nmod windows;\n#[cfg(target_os = "macos")]\nmod macos;\nconst U: bool = cfg!(unix);\n'
        self.write(f"{ENGINE}/src/platform/mod.rs", body)
        self.write(f"{ENGINE}/src/platform/windows.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.write(f"{ENGINE}/src/platform/linux/proc.rs", '#[cfg(target_env = "musl")]\nfn f() {}\n')
        self.write(f"{ENGINE}/src/tools/shell/platform/macos.rs", "#[cfg(unix)]\nfn f() {}\n")
        self.write(f"{DEVICE}/src/secure/files/platform.rs", body)
        self.write(f"{DEVICE}/src/a.rs", "mod platform {\n    #[cfg(windows)]\n    pub fn f() {}\n    mod inner {\n        #[cfg(unix)]\n        fn g() {}\n    }\n}\n")
        self.check([])

    def test_a_platform_name_that_is_no_module_fails(self):
        self.write(f"{ENGINE}/src/platform_windows.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.write(f"{ENGINE}/src/platforms/mod.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.write(f"{ENGINE}/platform/build.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.write(f"{ENGINE}/build.rs", "fn main() {\n    #[cfg(windows)]\n    f();\n}\n")
        self.write(f"{ENGINE}/src/a.rs", "mod platform {\n    pub fn f() {}\n}\n#[cfg(unix)]\nfn g() {}\nmod other {\n    #[cfg(unix)]\n    fn h() {}\n}\n")
        self.check([
            f"{ENGINE}/src/platform_windows.rs:1",
            f"{ENGINE}/src/platforms/mod.rs:1",
            f"{ENGINE}/platform/build.rs:1",
            f"{ENGINE}/build.rs:2",
            f"{ENGINE}/src/a.rs:4",
            f"{ENGINE}/src/a.rs:7",
        ])

    def test_test_feature_and_other_conditions_pass(self):
        self.write(
            f"{ENGINE}/src/a.rs",
            '#[cfg(test)]\nmod t;\n#[cfg(feature = "windows")]\nfn a() {}\n#[cfg(all(debug_assertions, not(feature = "unix")))]\nfn b() {}\n'
            '#[cfg(target_arch = "x86_64")]\nfn c() {}\n#[cfg(target_pointer_width = "64")]\nfn d() {}\n'
            '#[cfg_attr(feature = "serde", serde(rename = "windows"))]\nstruct S;\n#[cfg_attr(test, derive(Debug))]\nstruct T;\n'
            'const F: bool = cfg!(feature = "target_os");\n',
        )
        self.check([])

    def test_comments_strings_and_plain_names_pass(self):
        self.write(
            f"{ENGINE}/src/a.rs",
            "//! Built with `#[cfg(windows)]`.\n// #[cfg(unix)]\n/* cfg!(windows) /* #[cfg(unix)] */ */\n"
            'const S: &str = "#[cfg(windows)]";\nconst R: &str = r#"cfg!(target_os = "linux")"#;\n'
            "mod windows;\nuse crate::unix::Mode;\nfn cfg(windows: u8) {}\nfn g(unix: u8) { self::cfg(unix); x.cfg(unix); }\n"
            "const OS: &str = std::env::consts::OS;\n",
        )
        self.check([])

    def test_crates_outside_the_device_list_are_not_judged(self):
        self.write("crates/infra/database/Cargo.toml", '[package]\nname = "systemprompt-database"\n')
        self.write("crates/infra/database/src/lib.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.write("crates/tests/unit/domain/engine/src/lib.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.write("bin/bridge/src/main.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.check([])

    def test_target_directories_are_not_read(self):
        self.write(f"{ENGINE}/target/debug/build/out.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.check([])

    def test_findings_are_grouped_by_crate_with_counts(self):
        self.write(f"{ENGINE}/src/a.rs", "#[cfg(windows)]\nfn f() {}\n#[cfg(unix)]\nfn g() {}\n")
        self.write(f"{ENGINE}/src/b.rs", "#[cfg(windows)]\nfn f() {}\n")
        self.write(f"{DEVICE}/src/a.rs", "#[cfg(windows)]\nfn f() {}\n")
        lines = self.run_gate().stdout.splitlines()
        engine = lines.index("systemprompt-engine: 3 target-system condition(s) in 2 file(s)")
        device = lines.index("systemprompt-device: 1 target-system condition(s) in 1 file(s)")
        self.assertEqual([line.split(":")[0].strip() for line in lines[engine + 1:engine + 4]], [f"{ENGINE}/src/a.rs"] * 2 + [f"{ENGINE}/src/b.rs"])
        self.assertEqual(lines[device + 1].split(":")[0].strip(), f"{DEVICE}/src/a.rs")
        self.assertEqual(
            lines[-1],
            f"lint-platform-confinement: FAIL (4 target-system condition(s) outside a platform module in 2 of {len(CRATES)} device crates)",
        )

    def test_report_prints_the_same_findings_and_exits_zero(self):
        self.write(f"{ENGINE}/src/a.rs", "#[cfg(windows)]\nfn f() {}\n")
        failed, report = self.run_gate(), self.run_gate("--report")
        self.assertEqual(failed.returncode, 1, failed.stdout)
        self.assertEqual(report.returncode, 0, report.stdout + report.stderr)
        self.assertEqual(report.stdout.splitlines()[:-1], failed.stdout.splitlines()[:-1])
        self.assertTrue(report.stdout.splitlines()[-1].startswith("lint-platform-confinement: REPORT (1 "), report.stdout)

    def test_a_missing_device_crate_fails_in_both_modes(self):
        for name in CRATES:
            with self.subTest(name=name):
                manifest = self.base / crate_dir(name) / "Cargo.toml"
                manifest.rename(manifest.with_suffix(".away"))
                for flags in ((), ("--report",)):
                    result = self.run_gate(*flags)
                    self.assertEqual(result.returncode, 1, result.stdout)
                    self.assertIn(f"  {name}: a device crate has a manifest", result.stdout)
                manifest.with_suffix(".away").rename(manifest)

    def test_a_device_crate_without_src_fails(self):
        (self.base / ENGINE / "src" / "lib.rs").unlink()
        (self.base / ENGINE / "src").rmdir()
        self.check(["systemprompt-engine"])

    def test_an_unknown_flag_is_a_usage_error(self):
        result = self.run_gate("--allow")
        self.assertEqual(result.returncode, 2, result.stdout)
        self.assertIn("lint-platform-confinement.py [--report] [ROOT]", result.stderr)

    def test_the_device_list_is_the_closure_gates(self):
        self.assertNotIn('"systemprompt-', SCRIPT.read_text())
        self.assertIn("systemprompt-engine", CRATES)
        self.assertIn("systemprompt-device", CRATES)


if __name__ == "__main__":
    unittest.main()
