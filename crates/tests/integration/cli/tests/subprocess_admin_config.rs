//! Subprocess coverage for the `admin config` tree, focused on the
//! rate-limits subcommands (show/tier/docs/set/compare/diff/preset/export/
//! import/enable/disable/reset) plus the per-section config views.
//!
//! Rate-limit mutations operate on the fixture's tempdir profile, so no
//! shared state is touched. Tests accept success or failure exit codes.

use systemprompt_cli_integration_tests::full_bootstrap::{fixture, run, run_with_formats};

#[test]
fn rate_limits_show_and_docs() {
    run_with_formats(&["admin", "config", "rate-limits", "show"]);
    run(&["admin", "config", "rate-limits", "docs"]);
}

#[test]
fn rate_limits_tiers() {
    for tier in ["admin", "user", "a2a", "mcp", "service", "anon"] {
        run(&["admin", "config", "rate-limits", "tier", tier]);
    }
    run(&["admin", "config", "rate-limits", "tier", "not_a_tier"]);
}

#[test]
fn rate_limits_compare_and_validate() {
    run_with_formats(&["admin", "config", "rate-limits", "compare"]);
    run_with_formats(&["admin", "config", "rate-limits", "validate"]);
}

#[test]
fn rate_limits_diff_defaults() {
    run(&["admin", "config", "rate-limits", "diff", "--defaults"]);
    run(&["admin", "config", "rate-limits", "diff"]);
}

#[test]
fn rate_limits_presets() {
    run(&["admin", "config", "rate-limits", "preset", "list"]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "preset",
        "show",
        "development",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "preset",
        "show",
        "production",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "preset",
        "show",
        "high-traffic",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "preset",
        "show",
        "no_such_preset",
    ]);
}

#[test]
fn rate_limits_set_variants() {
    run(&[
        "admin",
        "config",
        "rate-limits",
        "set",
        "--endpoint",
        "contexts",
        "--rate",
        "42",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "set",
        "--tier",
        "user",
        "--multiplier",
        "2.5",
    ]);
    run(&["admin", "config", "rate-limits", "set", "--burst", "4"]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "set",
        "--endpoint",
        "bogus",
        "--rate",
        "1",
    ]);
    run(&["admin", "config", "rate-limits", "set"]);
}

#[test]
fn rate_limits_enable_disable_reset() {
    run(&["admin", "config", "rate-limits", "enable"]);
    run(&["admin", "config", "rate-limits", "disable"]);
    run(&["admin", "config", "rate-limits", "reset"]);
}

#[test]
fn rate_limits_reset_scoped_variants() {
    run(&[
        "admin",
        "config",
        "rate-limits",
        "set",
        "--endpoint",
        "mcp",
        "--rate",
        "999",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "reset",
        "--endpoint",
        "mcp",
        "--dry-run",
    ]);
    run_with_formats(&[
        "admin",
        "config",
        "rate-limits",
        "reset",
        "--endpoint",
        "mcp",
        "--yes",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "reset",
        "--endpoint",
        "mcp",
        "--yes",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "set",
        "--tier",
        "anon",
        "--multiplier",
        "7.5",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "reset",
        "--tier",
        "anon",
        "--dry-run",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "reset",
        "--tier",
        "anon",
        "--yes",
    ]);
    run(&["admin", "config", "rate-limits", "reset", "--dry-run"]);
    run(&["admin", "config", "rate-limits", "reset", "--yes"]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "reset",
        "--endpoint",
        "bogus_endpoint",
        "--yes",
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "reset",
        "--tier",
        "bogus_tier",
        "--yes",
    ]);
}

#[test]
fn rate_limits_export_import_roundtrip() {
    let Some(fixture) = fixture() else { return };
    let path = fixture.system_dir.join("rate_limits_export.yaml");
    let path_str = path.to_string_lossy().into_owned();
    run(&[
        "admin",
        "config",
        "rate-limits",
        "export",
        "--output",
        &path_str,
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "import",
        "--file",
        &path_str,
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "diff",
        "--file",
        &path_str,
    ]);
    run(&[
        "admin",
        "config",
        "rate-limits",
        "import",
        "--file",
        "/nonexistent.yaml",
    ]);
}

#[test]
fn config_sections_with_formats() {
    run_with_formats(&["admin", "config", "provider"]);
    run_with_formats(&["admin", "config", "runtime"]);
    run_with_formats(&["admin", "config", "security"]);
    run_with_formats(&["admin", "config", "server"]);
    run_with_formats(&["admin", "config", "paths"]);
    run_with_formats(&["admin", "config", "validate"]);
    run_with_formats(&["admin", "config", "list"]);
    run_with_formats(&["admin", "config", "show"]);
}
