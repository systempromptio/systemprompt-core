//! `infra services serve --skip-migrate` and the effective boot policy.
//!
//! The flag can only move the policy towards skipping: a profile that sets
//! `database.migrate_on_boot: false` is not overridden back on.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::services::ServicesCommands;
use systemprompt_cli::infrastructure::services::serve::effective_run_migrations;

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
fn serve_accepts_skip_migrate_with_foreground() {
    match parse(&["serve", "--skip-migrate", "--foreground"]).expect("parses") {
        ServicesCommands::Serve {
            foreground,
            skip_migrate,
            kill_port_process,
        } => {
            assert!(foreground);
            assert!(skip_migrate);
            assert!(!kill_port_process);
        },
        other => panic!("wrong subcommand: {other:?}"),
    }
}

#[test]
fn serve_migrates_unless_told_otherwise() {
    match parse(&["serve"]).expect("parses") {
        ServicesCommands::Serve { skip_migrate, .. } => assert!(!skip_migrate),
        other => panic!("wrong subcommand: {other:?}"),
    }
}

#[test]
fn effective_policy_only_moves_towards_skipping() {
    assert!(effective_run_migrations(false, true));
    assert!(!effective_run_migrations(true, true));
    assert!(!effective_run_migrations(false, false));
    assert!(!effective_run_migrations(true, false));
}
