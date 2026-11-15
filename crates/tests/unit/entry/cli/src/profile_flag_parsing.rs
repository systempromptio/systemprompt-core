//! A subcommand `--profile` shares clap's value slot with the global
//! `--profile`, so parsing the whole `Cli` with one must succeed and leave the
//! same name in both places.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::AdminCommands;
use systemprompt_cli::admin::session::SessionCommands;
use systemprompt_cli::args::{Cli, Commands};

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
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
    let cli = parse(&[
        "--profile",
        "/tmp/profiles/prod",
        "admin",
        "session",
        "show",
    ])
    .expect("the global flag resolves names and paths");
    assert_eq!(
        cli.profile_opts.profile.as_deref(),
        Some("/tmp/profiles/prod")
    );
}
