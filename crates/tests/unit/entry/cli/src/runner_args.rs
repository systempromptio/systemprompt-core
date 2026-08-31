//! The argument plane: config assembly, argv reconstruction, and the
//! export-flag check.
//!
//! `reconstruct_args` is what a profile-routed command sends to the subprocess
//! it re-invokes, so a flag lost or duplicated here changes what actually runs
//! on the far side. The reconstruction used to read `std::env::args()` inside
//! itself, which meant its branches were reachable only through however the
//! test binary happened to be invoked; it now takes the original argv, and
//! these drive it directly.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::args::{
    Cli, build_cli_config, has_local_export_flag_in, reconstruct_args_from,
};
use systemprompt_cli::{ColorMode, EnvOverrides, OutputFormat, VerbosityLevel};

fn cli(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
}

fn parse_fails(args: &[&str]) -> bool {
    Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied())).is_err()
}

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn each_verbosity_flag_reaches_the_config() {
    let env = EnvOverrides::default();

    for (flag, expected) in [
        ("--debug", VerbosityLevel::Debug),
        ("--verbose", VerbosityLevel::Verbose),
        ("--quiet", VerbosityLevel::Quiet),
    ] {
        assert_eq!(build_cli_config(&cli(&[flag]), &env).verbosity, expected);
    }

    assert_eq!(
        build_cli_config(&cli(&[]), &env).verbosity,
        VerbosityLevel::Normal,
        "no flag leaves the default"
    );
}

// Why: only `--quiet` declares a conflict, and only with `--verbose`.
// `--debug` conflicts with neither, so it is genuinely reachable alongside
// them and the if/else ordering in `build_cli_config` is what makes it win.
// Reorder that chain and `--quiet --debug` silently yields Quiet — dropping
// debug logging at the moment someone asked for it.
#[test]
fn debug_wins_over_the_flags_it_does_not_conflict_with() {
    let env = EnvOverrides::default();

    assert_eq!(
        build_cli_config(&cli(&["--verbose", "--debug"]), &env).verbosity,
        VerbosityLevel::Debug
    );
    assert_eq!(
        build_cli_config(&cli(&["--quiet", "--debug"]), &env).verbosity,
        VerbosityLevel::Debug
    );
    assert!(
        parse_fails(&["--quiet", "--verbose"]),
        "these two are the only pair clap rejects"
    );
}

#[test]
fn each_output_flag_reaches_the_config_and_the_two_conflict() {
    let env = EnvOverrides::default();

    assert_eq!(
        build_cli_config(&cli(&["--json"]), &env).output_format,
        OutputFormat::Json
    );
    assert_eq!(
        build_cli_config(&cli(&["--yaml"]), &env).output_format,
        OutputFormat::Yaml
    );
    assert_eq!(
        build_cli_config(&cli(&[]), &env).output_format,
        OutputFormat::Table,
        "no flag leaves the default"
    );
    assert!(
        parse_fails(&["--json", "--yaml"]),
        "asking for two output formats is a parse error, not a precedence question"
    );
}

#[test]
fn display_flags_reach_the_config() {
    let env = EnvOverrides::default();
    let cfg = build_cli_config(&cli(&["--no-color", "--non-interactive"]), &env);

    assert_eq!(cfg.color_mode, ColorMode::Never);
    assert!(!cfg.interactive);
}

#[test]
fn the_profile_override_is_carried_through() {
    let env = EnvOverrides::default();
    let cfg = build_cli_config(&cli(&["--profile", "staging"]), &env);

    assert_eq!(cfg.profile_override.as_deref(), Some("staging"));
}

// Why: the reconstructed argv is what the re-invoked subprocess receives. A
// global flag appearing twice would be passed twice, and clap rejects a
// repeated flag — so the deduplication is what keeps profile routing working
// at all.
#[test]
fn a_global_flag_is_not_repeated_when_it_was_already_on_the_command_line() {
    let parsed = cli(&["--json", "--debug"]);
    let out = reconstruct_args_from(&parsed, &owned(&["--json", "--debug", "admin", "users"]));

    assert_eq!(
        out.iter().filter(|a| *a == "--json").count(),
        1,
        "--json must appear once, got {out:?}"
    );
    assert_eq!(
        out.iter().filter(|a| *a == "--debug").count(),
        1,
        "--debug must appear once, got {out:?}"
    );
    assert!(out.contains(&"admin".to_owned()));
    assert!(out.contains(&"users".to_owned()));
}

// Why: the subprocess is being routed to a *different* profile, so carrying
// the original `--profile` through would send it back to the one it came from.
// Both spellings have to be dropped.
#[test]
fn the_original_profile_flag_is_dropped_in_both_spellings() {
    let parsed = cli(&["--profile", "prod"]);

    let spaced = reconstruct_args_from(
        &parsed,
        &owned(&["--profile", "prod", "core", "skills", "list"]),
    );
    let equals = reconstruct_args_from(
        &parsed,
        &owned(&["--profile=prod", "core", "skills", "list"]),
    );

    for out in [&spaced, &equals] {
        assert_eq!(
            out.iter().filter(|a| *a == "prod").count(),
            1,
            "the profile value should survive exactly once, from the parsed \
             flag rather than the raw argv: {out:?}"
        );
        assert_eq!(
            out.iter().filter(|a| a.starts_with("--profile")).count(),
            1,
            "exactly one --profile flag should be emitted; the raw argv's copy, \
             in either spelling, must not be carried through: {out:?}"
        );
        assert!(out.contains(&"core".to_owned()));
        assert!(out.contains(&"skills".to_owned()));
        assert!(out.contains(&"list".to_owned()));
    }
}

#[test]
fn short_verbosity_spellings_are_not_passed_through_raw() {
    let parsed = cli(&["-v"]);
    let out = reconstruct_args_from(&parsed, &owned(&["-v", "core", "skills", "list"]));

    assert!(
        !out.contains(&"-v".to_owned()),
        "the short form should be re-emitted as its long form, not duplicated: {out:?}"
    );
    assert!(out.contains(&"--verbose".to_owned()));
}

#[test]
fn positional_arguments_survive_in_order() {
    let parsed = cli(&["core", "skills", "list"]);
    let out = reconstruct_args_from(&parsed, &owned(&["core", "skills", "list"]));

    assert_eq!(out, vec!["core", "skills", "list"]);
}

// Why: the export flag only means anything for `analytics`. Treating it as
// local for any other command would route a subprocess call back to the local
// profile and silently run it against the wrong database.
#[test]
fn the_export_flag_is_only_local_for_analytics() {
    let analytics = cli(&["analytics", "overview"]);
    let other = cli(&["core", "skills", "list"]);
    let args = owned(&["systemprompt", "analytics", "overview", "--export"]);

    assert!(has_local_export_flag_in(analytics.command.as_ref(), &args));
    assert!(
        !has_local_export_flag_in(other.command.as_ref(), &args),
        "a non-analytics command must not be treated as a local export"
    );
    assert!(
        !has_local_export_flag_in(None, &args),
        "no command at all is not an export"
    );
}

#[test]
fn the_export_flag_is_recognised_with_and_without_a_value() {
    let analytics = cli(&["analytics", "overview"]);

    assert!(has_local_export_flag_in(
        analytics.command.as_ref(),
        &owned(&["systemprompt", "--export"])
    ));
    assert!(has_local_export_flag_in(
        analytics.command.as_ref(),
        &owned(&["systemprompt", "--export=/tmp/out.csv"])
    ));
    assert!(
        !has_local_export_flag_in(
            analytics.command.as_ref(),
            &owned(&["systemprompt", "--exported"])
        ),
        "a longer flag that merely starts the same way is not --export"
    );
}
