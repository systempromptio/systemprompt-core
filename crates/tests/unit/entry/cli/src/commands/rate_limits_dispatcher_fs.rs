//! Tests for the `admin config rate-limits` command tree against the
//! bootstrapped profile.
//!
//! The read-only arms (show/validate) and the editing arms
//! (set/enable/disable/reset/import/export) all resolve the active profile
//! path; the fixture profile lives in a tempdir, so the editing arms can be
//! asserted against the YAML they rewrite.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::config::rate_limits::{RateLimitsCommands, execute};
use systemprompt_cli::{CliConfig, OutputFormat, ScriptedPrompter};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: RateLimitsCommands,
}

fn parse(args: &[&str]) -> RateLimitsCommands {
    Harness::try_parse_from(std::iter::once("rate-limits").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn config() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

fn profile_path() -> PathBuf {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .profile_path
        .clone()
}

fn run(args: &[&str]) -> anyhow::Result<()> {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    execute(parse(args), &prompter, &config())
}

fn profile_yaml() -> String {
    std::fs::read_to_string(profile_path()).unwrap()
}

#[test]
fn read_only_arms_render_the_active_configuration() {
    profile_path();

    run(&["show"]).unwrap();
    run(&["docs"]).unwrap();
    run(&["compare"]).unwrap();
    run(&["validate"]).unwrap();
    for tier in ["admin", "user", "a2a", "mcp", "service", "anon"] {
        run(&["tier", tier]).unwrap();
    }
}

#[test]
fn set_requires_its_flags_in_matching_pairs() {
    profile_path();

    let cases = [
        (vec!["set", "--endpoint", "tasks"], "--rate is required"),
        (vec!["set", "--rate", "5"], "--endpoint is required"),
        (vec!["set", "--tier", "admin"], "--multiplier is required"),
        (vec!["set", "--multiplier", "2.0"], "--tier is required"),
        (vec!["set"], "Must specify one of"),
    ];

    for (args, expected) in cases {
        let err = run(&args).unwrap_err();
        assert!(format!("{err:#}").contains(expected), "{args:?}");
    }
}

#[test]
fn set_writes_an_endpoint_rate_into_the_profile() {
    profile_path();

    run(&["set", "--endpoint", "tasks", "--rate", "77"]).unwrap();
    assert!(profile_yaml().contains("77"), "{}", profile_yaml());
}

#[test]
fn disable_then_enable_round_trips_the_flag_in_the_profile() {
    profile_path();

    run(&["disable"]).unwrap();
    assert!(
        profile_yaml().contains("disabled: true"),
        "{}",
        profile_yaml()
    );

    run(&["enable"]).unwrap();
    assert!(
        profile_yaml().contains("disabled: false"),
        "{}",
        profile_yaml()
    );
}

#[test]
fn export_writes_a_file_that_import_reads_back() {
    profile_path();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("limits.yaml");

    // `export` serialises the runtime config the process booted with, not the
    // profile file, so the round-trip is asserted on the imported document.
    run(&["export", "-o", out.to_str().unwrap()]).unwrap();
    let exported = std::fs::read_to_string(&out).unwrap();
    assert!(exported.contains("tasks_per_second"), "{exported}");

    let edited = exported.replace("tasks_per_second:", "tasks_per_second: 137 #");
    std::fs::write(&out, &edited).unwrap();

    run(&["import", "-f", out.to_str().unwrap(), "--yes"]).unwrap();
    assert!(profile_yaml().contains("137"), "{}", profile_yaml());
}

#[test]
fn import_rejects_a_missing_file() {
    profile_path();

    let err = run(&["import", "-f", "/nonexistent/limits.yaml", "--yes"]).unwrap_err();
    assert!(!format!("{err:#}").is_empty());
}

#[test]
fn diff_against_defaults_reports_the_edited_endpoint() {
    profile_path();

    run(&["set", "--endpoint", "tasks", "--rate", "321"]).unwrap();
    run(&["diff", "--defaults"]).unwrap();
}

#[test]
fn reset_restores_a_single_endpoint_to_its_default() {
    profile_path();

    run(&["set", "--endpoint", "tasks", "--rate", "654"]).unwrap();
    assert!(profile_yaml().contains("654"));

    run(&["reset", "--endpoint", "tasks", "--yes"]).unwrap();
    assert!(!profile_yaml().contains("654"), "{}", profile_yaml());
}

#[test]
fn reset_dry_run_leaves_the_profile_untouched() {
    profile_path();

    run(&["set", "--endpoint", "tasks", "--rate", "888"]).unwrap();
    run(&["reset", "--dry-run", "--yes"]).unwrap();
    assert!(profile_yaml().contains("888"), "{}", profile_yaml());
}

#[test]
fn applying_a_preset_rewrites_the_whole_block() {
    profile_path();

    run(&["preset", "apply", "high-traffic", "--yes"]).unwrap();

    let yaml = profile_yaml();
    assert!(yaml.contains("rate_limits"), "{yaml}");
}
