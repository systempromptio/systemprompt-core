//! A subcommand `--profile` shares clap's value slot with the global
//! `--profile`, so parsing the whole `Cli` with one must succeed and leave the
//! same name in both places.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::AdminCommands;
use systemprompt_cli::admin::session::SessionCommands;
use systemprompt_cli::args::{Cli, Commands};
use systemprompt_cli::cloud::CloudCommands;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
}

fn cloud_profile(cli: &Cli) -> Option<&str> {
    match cli.command.as_ref() {
        Some(Commands::Cloud(
            CloudCommands::Doctor { profile, .. }
            | CloudCommands::Deploy { profile, .. }
            | CloudCommands::Backup { profile, .. },
        )) => profile.as_deref(),
        other => panic!("expected a cloud doctor, deploy or backup command, got {other:?}"),
    }
}

#[test]
fn cloud_profile_flags_parse_alongside_the_global_flag() {
    for args in [
        ["cloud", "doctor", "--profile", "prod"],
        ["cloud", "deploy", "--profile", "prod"],
        ["cloud", "backup", "--profile", "prod"],
        ["cloud", "doctor", "-p", "prod"],
    ] {
        let cli = parse(&args).unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
        assert_eq!(cloud_profile(&cli), Some("prod"), "{args:?}");
        assert_eq!(
            cli.profile_opts.profile.as_deref(),
            Some("prod"),
            "{args:?} must also select the profile the CLI bootstraps"
        );
    }
}

#[test]
fn cloud_profile_flags_refuse_a_path() {
    for args in [
        ["cloud", "doctor", "--profile", "/tmp/profiles/prod"],
        ["cloud", "deploy", "--profile", "profiles/prod"],
        ["cloud", "backup", "--profile", "prod name"],
    ] {
        let err = parse(&args).expect_err("a profile path is a usage error");
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::ValueValidation,
            "{args:?}"
        );
    }
}

#[test]
fn session_logout_profile_flag_parses_alongside_the_global_flag() {
    let cli = parse(&["admin", "session", "logout", "--profile", "staging", "-y"])
        .expect("logout --profile parses");
    let Some(Commands::Admin(AdminCommands::Session(SessionCommands::Logout(args)))) =
        cli.command.as_ref()
    else {
        panic!("expected admin session logout");
    };
    assert_eq!(args.profile.as_deref(), Some("staging"));
    assert_eq!(cli.profile_opts.profile.as_deref(), Some("staging"));
}

#[test]
fn the_global_profile_flag_still_accepts_a_path() {
    let cli = parse(&["--profile", "/tmp/profiles/prod", "cloud", "status"])
        .expect("the global flag resolves names and paths");
    assert_eq!(
        cli.profile_opts.profile.as_deref(),
        Some("/tmp/profiles/prod")
    );
}
