//! Tests for how much bootstrap each `cloud` subcommand declares it needs.
//!
//! `DescribeCommand::descriptor` is the single source of truth the runner reads
//! to decide whether to load a profile, resolve secrets, or neither. Getting it
//! wrong either boots a command without a profile it needs, or makes a command
//! that should run before any profile exists (`init`, `dockerfile`) fail on a
//! missing one.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::cloud::CloudCommands;
use systemprompt_cli::descriptor::DescribeCommand;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: CloudCommands,
}

fn parse(args: &[&str]) -> CloudCommands {
    Harness::try_parse_from(std::iter::once("cloud").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("failed to parse {args:?}: {e}"))
        .cmd
}

#[test]
fn deploy_needs_a_profile_and_secrets() {
    for args in [vec!["deploy"]] {
        let desc = parse(&args).descriptor();
        assert!(desc.profile(), "{args:?} should load a profile");
        assert!(desc.secrets(), "{args:?} should resolve secrets");
    }
}

#[test]
fn status_and_backup_need_a_profile_only() {
    for args in [vec!["status"], vec!["backup"]] {
        let desc = parse(&args).descriptor();
        assert!(desc.profile(), "{args:?} should load a profile");
        assert!(
            !desc.secrets(),
            "{args:?} reads no secrets, so it should not pay to resolve them"
        );
    }
}

#[test]
fn init_and_dockerfile_need_no_bootstrap() {
    for args in [vec!["init"], vec!["dockerfile"]] {
        let desc = parse(&args).descriptor();
        assert!(
            !desc.profile(),
            "{args:?} must be runnable before a profile exists"
        );
        assert!(!desc.secrets());
    }
}

#[test]
fn resolving_secrets_always_implies_loading_a_profile() {
    for args in [
        vec!["status"],
        vec!["backup"],
        vec!["init"],
        vec!["dockerfile"],
        vec!["deploy"],
        vec!["auth", "status"],
    ] {
        let desc = parse(&args).descriptor();
        if desc.secrets() {
            assert!(
                desc.profile(),
                "{args:?} asks for secrets but not a profile; secrets are resolved \
                 relative to the profile, so this combination cannot boot"
            );
        }
    }
}
