//! The argument plane: config assembly, argv reconstruction, and the
//! export-flag check.
//!
//! `reconstruct_args` is what a profile-routed command sends to the remote
//! tenant, so a token lost or duplicated here changes what actually runs on
//! the far side. It edits the original argv by position; it used to drop any
//! token already emitted, which collapsed `admin agents show admin` and
//! `--limit 10 --offset 10`.

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

fn forwarded(args: &[&str]) -> Vec<String> {
    reconstruct_args_from(&owned(args))
}

#[test]
fn global_flags_typed_first_follow_the_top_level_group() {
    assert_eq!(
        forwarded(&["--json", "--debug", "admin", "users", "list"]),
        vec!["admin", "--json", "--debug", "users", "list"],
        "the gateway requires a subcommand first; globals stay valid after the group"
    );
}

#[test]
fn global_flags_after_the_subcommand_stay_where_they_were() {
    assert_eq!(
        forwarded(&["infra", "logs", "show", "abc", "--json", "-v"]),
        vec!["infra", "logs", "show", "abc", "--json", "-v"],
        "a flag after the leaf may be the leaf's own `--json`; it must reach the leaf"
    );
}

#[test]
fn the_profile_flag_is_dropped_in_both_spellings_wherever_it_appears() {
    for args in [
        vec!["--profile", "prod", "core", "skills", "list"],
        vec!["--profile=prod", "core", "skills", "list"],
        vec!["core", "skills", "list", "--profile", "prod"],
        vec!["core", "skills", "--profile=prod", "list"],
    ] {
        assert_eq!(
            forwarded(&args),
            vec!["core", "skills", "list"],
            "{args:?}: the local profile name means nothing on the tenant"
        );
    }
}

#[test]
fn the_database_url_flag_is_never_forwarded() {
    for args in [
        vec![
            "--database-url",
            "postgres://u:p@h/db",
            "admin",
            "users",
            "list",
        ],
        vec![
            "admin",
            "users",
            "list",
            "--database-url=postgres://u:p@h/db",
        ],
    ] {
        let out = forwarded(&args);
        assert_eq!(out, vec!["admin", "users", "list"], "{args:?}");
        assert!(!out.iter().any(|a| a.contains("postgres://")), "{out:?}");
    }
}

#[test]
fn a_positional_equal_to_a_subcommand_word_survives() {
    assert_eq!(
        forwarded(&["admin", "agents", "show", "admin"]),
        vec!["admin", "agents", "show", "admin"]
    );
}

#[test]
fn repeated_values_are_forwarded_once_each_in_order() {
    assert_eq!(
        forwarded(&[
            "infra", "db", "query", "SELECT 1", "--limit", "10", "--offset", "10"
        ]),
        vec![
            "infra", "db", "query", "SELECT 1", "--limit", "10", "--offset", "10"
        ]
    );
    assert_eq!(
        forwarded(&["--profile", "prod", "cloud", "tenant", "show", "prod"]),
        vec!["cloud", "tenant", "show", "prod"],
        "only the profile flag's own value is dropped, not an equal positional"
    );
}

#[test]
fn tokens_after_a_terminator_are_never_edited() {
    assert_eq!(
        forwarded(&["plugins", "run", "ext", "--", "--profile", "x"]),
        vec!["plugins", "run", "ext", "--", "--profile", "x"]
    );
}

#[test]
fn positional_arguments_survive_in_order() {
    assert_eq!(
        forwarded(&["core", "skills", "list"]),
        vec!["core", "skills", "list"]
    );
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
