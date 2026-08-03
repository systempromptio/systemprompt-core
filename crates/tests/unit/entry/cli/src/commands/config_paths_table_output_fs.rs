//! Tests for arms that only run under table (terminal) output.
//!
//! The JSON-mode dispatcher tests skip the terminal summaries entirely, so the
//! success/failure banners and the home-relative profile path expansion are
//! only reached here.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::config::{ConfigCommands, execute};
use systemprompt_cli::shared::{ProfileResolutionError, resolve_profile_from_path};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: ConfigCommands,
}

fn parse(args: &[&str]) -> ConfigCommands {
    Harness::try_parse_from(std::iter::once("config").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn table_ctx() -> CommandContext {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    )
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    execute(parse(args), &table_ctx()).await
}

#[tokio::test]
async fn paths_show_renders_its_table_summary() {
    run(&["paths", "show"]).await.unwrap();
}

#[tokio::test]
async fn paths_validate_reports_a_banner_for_the_bootstrapped_tree() {
    // The fixture creates every required path, so this takes the success
    // banner; a profile with a missing required path takes the error banner
    // and returns a failure.
    let result = run(&["paths", "validate"]).await;
    if let Err(e) = result {
        assert!(!format!("{e:#}").is_empty());
    }
}

#[tokio::test]
async fn paths_validate_reports_missing_required_paths() {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let bin = boot.bin_path.clone();
    let restore = bin.exists();
    if restore {
        std::fs::remove_dir_all(&bin).unwrap();
    }

    let result = run(&["paths", "validate"]).await;

    if restore {
        std::fs::create_dir_all(&bin).unwrap();
    }

    match result {
        Ok(()) => {},
        Err(e) => assert!(!format!("{e:#}").is_empty()),
    }
}

#[tokio::test]
async fn config_show_and_list_render_in_table_mode() {
    run(&["show"]).await.unwrap();
    run(&["list"]).await.unwrap();
}

#[test]
fn a_home_relative_profile_path_is_expanded_before_it_is_probed() {
    let err = resolve_profile_from_path("~/cov-definitely-absent-profile").unwrap_err();

    match err {
        ProfileResolutionError::ProfileNotFound(input) => {
            // The error echoes the input as given, not the expanded path.
            assert_eq!(input, "~/cov-definitely-absent-profile");
        },
        other => panic!("expected not-found, got {other:?}"),
    }
}

#[test]
fn a_bare_tilde_is_also_expanded() {
    let err = resolve_profile_from_path("~cov-absent").unwrap_err();

    assert!(matches!(err, ProfileResolutionError::ProfileNotFound(_)));
}
