//! `core services` subcommand parsing.
//!
//! The group is the CI entry point for bundle authoring, so the flag names
//! consumers script against are pinned here, including the pair `inspect`
//! refuses to accept together.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::services::ServicesCommands;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: ServicesCommands,
}

fn parse(args: &[&str]) -> Result<ServicesCommands, clap::Error> {
    Harness::try_parse_from(std::iter::once("services").chain(args.iter().copied()))
        .map(|h| h.command)
}

#[test]
fn validate_takes_a_root_a_base_and_a_previous_bundle() {
    let command = parse(&[
        "validate",
        "--root",
        "services",
        "--base",
        "base.tar.gz",
        "--against",
        "prev.tar.gz",
        "--strict",
    ])
    .expect("parses");
    match command {
        ServicesCommands::Validate(args) => {
            assert_eq!(args.root, std::path::PathBuf::from("services"));
            assert!(args.base.is_some());
            assert!(args.against.is_some());
            assert!(args.strict);
        },
        other => panic!("wrong subcommand: {other:?}"),
    }
}

#[test]
fn refresh_defaults_to_swapping_and_opts_into_checking() {
    match parse(&["refresh"]).expect("parses") {
        ServicesCommands::Refresh(args) => assert!(!args.check),
        other => panic!("wrong subcommand: {other:?}"),
    }
    match parse(&["refresh", "--check"]).expect("parses") {
        ServicesCommands::Refresh(args) => assert!(args.check),
        other => panic!("wrong subcommand: {other:?}"),
    }
}

#[test]
fn inspect_refuses_an_archive_and_the_active_root_together() {
    assert!(parse(&["inspect", "--active"]).is_ok());
    assert!(parse(&["inspect", "--bundle", "b.tar.gz"]).is_ok());
    assert!(
        parse(&["inspect", "--bundle", "b.tar.gz", "--active"]).is_err(),
        "the two modes must conflict"
    );
}

#[test]
fn bundle_and_publish_require_their_destinations() {
    assert!(parse(&["bundle", "--root", "services"]).is_err());
    assert!(parse(&["publish", "--bundle", "b.tar.gz"]).is_err());
    assert!(
        parse(&[
            "publish",
            "--bundle",
            "b.tar.gz",
            "--to",
            "oci://ghcr.io/org/s:1.0.0"
        ])
        .is_ok()
    );
}

#[test]
fn an_unknown_subcommand_is_refused() {
    assert!(parse(&["teleport"]).is_err());
}
