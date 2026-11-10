//! Irreversible commands are confirmed once, by the runner, from their
//! compile-time `DataImpact` declaration: on a terminal the operator is
//! asked, without one the command refuses unless `--yes` was passed, and a
//! remote route carries the answer as `--yes`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::{CommandFactory, FromArgMatches};
use systemprompt_cli::args::{Cli, Commands};
use systemprompt_cli::descriptor::{CommandDescriptor, DataImpact, DescribeCommand};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::runner::confirmation::{
    ConfirmationRequest, confirm_irreversible, forward_with_yes,
};
use systemprompt_cli::{CliConfig, VerbosityLevel};

const IRREVERSIBLE: &[&[&str]] = &[
    &["infra", "db", "migrate-down", "users", "1"],
    &[
        "infra",
        "db",
        "migrate-mark-applied",
        "--extension",
        "users",
        "--version",
        "1",
    ],
    &["infra", "db", "assign-admin", "u"],
    &["admin", "users", "role", "assign", "u", "--roles", "admin"],
    &["admin", "users", "role", "promote", "u"],
    &["admin", "users", "role", "demote", "u"],
    &["admin", "bridge", "rotate-signing-key"],
    &["admin", "keys", "stage"],
    &["admin", "keys", "promote"],
    &["admin", "keys", "promote", "--revoke"],
    &["admin", "keys", "rotate"],
    &["admin", "keys", "rotate", "--revoke"],
    &["admin", "users", "federated-rekey", "--apply"],
];

fn parsed(args: &[&str]) -> (CommandDescriptor, ConfirmationRequest) {
    let argv: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let matches = Cli::command()
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
    let cli = Cli::from_arg_matches(&matches).expect("typed parse");
    let desc = cli
        .command
        .as_ref()
        .map_or(CommandDescriptor::FULL, Commands::descriptor);
    let request = ConfirmationRequest::from_matches(&matches, &argv, Some("prod".to_owned()));
    (desc, request)
}

fn with_yes<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut v = args.to_vec();
    v.push("--yes");
    v
}

fn config(interactive: bool) -> CliConfig {
    CliConfig::default()
        .with_interactive(interactive)
        .with_assume_terminal(interactive)
        .with_verbosity(VerbosityLevel::Quiet)
}

#[test]
fn every_irreversible_command_is_declared_so_and_accepts_yes() {
    for args in IRREVERSIBLE {
        let (desc, request) = parsed(args);
        assert_eq!(desc.data_impact(), DataImpact::Irreversible, "{args:?}");
        assert!(
            desc.is_destructive(),
            "{args:?} must keep the implicit-cloud refusal"
        );
        assert!(!request.yes, "{args:?}");

        let (_, confirmed) = parsed(&with_yes(args));
        assert!(
            confirmed.yes,
            "{args:?} --yes was not read from the leaf command"
        );
    }
}

#[test]
fn without_a_terminal_an_irreversible_command_refuses_unless_yes_is_passed() {
    let prompter = ScriptedPrompter::default();
    for args in IRREVERSIBLE {
        let (desc, request) = parsed(args);
        let err = confirm_irreversible(&desc, &request, &config(false), &prompter)
            .expect_err("no terminal and no --yes must refuse");
        let message = format!("{err:#}");
        assert!(message.contains("--yes"), "{message}");
        assert!(message.contains("prod"), "{message}");

        let (desc, request) = parsed(&with_yes(args));
        confirm_irreversible(&desc, &request, &config(false), &prompter)
            .expect("--yes is the non-interactive consent");
    }
}

#[test]
fn on_a_terminal_the_operator_answer_decides() {
    let (desc, request) = parsed(IRREVERSIBLE[0]);

    let declined = ScriptedPrompter::new(["n"]);
    let err = confirm_irreversible(&desc, &request, &config(true), &declined)
        .expect_err("a declined prompt must stop the command");
    assert!(format!("{err:#}").contains("cancelled"));

    let accepted = ScriptedPrompter::new(["y"]);
    confirm_irreversible(&desc, &request, &config(true), &accepted).expect("accepted");
}

#[test]
fn commands_that_are_not_irreversible_are_never_prompted() {
    let exhausted = ScriptedPrompter::default();
    for args in [
        vec!["infra", "db", "migrate"],
        vec!["infra", "db", "tables"],
        vec!["core", "services", "refresh"],
        vec!["admin", "users", "federated-rekey"],
        vec![
            "admin",
            "config",
            "secret",
            "migrate-git-credentials",
            "--apply",
        ],
    ] {
        let (desc, request) = parsed(&args);
        assert!(!desc.requires_confirmation(), "{args:?}");
        confirm_irreversible(&desc, &request, &config(false), &exhausted)
            .unwrap_or_else(|e| panic!("{args:?} was gated: {e:#}"));
    }
}

#[test]
fn a_remote_route_carries_the_confirmation_as_yes() {
    let (desc, _) = parsed(IRREVERSIBLE[0]);
    let forwarded = forward_with_yes(
        &desc,
        vec![
            "infra".into(),
            "db".into(),
            "migrate-down".into(),
            "users".into(),
            "1".into(),
        ],
    );
    assert_eq!(forwarded.last().map(String::as_str), Some("--yes"));

    let terminated = forward_with_yes(&desc, vec!["infra".into(), "--".into(), "x".into()]);
    assert_eq!(terminated, ["infra", "--yes", "--", "x"]);

    let already = forward_with_yes(&desc, vec!["infra".into(), "-y".into()]);
    assert_eq!(already, ["infra", "-y"]);

    let (preserving, _) = parsed(&["infra", "db", "tables"]);
    assert_eq!(
        forward_with_yes(&preserving, vec!["infra".into()]),
        ["infra"]
    );
}
