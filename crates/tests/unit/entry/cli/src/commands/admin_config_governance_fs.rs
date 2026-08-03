//! Tests for `admin config governance set`.
//!
//! Each authz mode has its own precondition, and the accepted ones rewrite the
//! profile's governance block.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::config::{ConfigCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

const UNRESTRICTED_ACK: &str = "I understand this disables all authorization";

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

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

fn profile_path() -> PathBuf {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .profile_path
        .clone()
}

fn profile_yaml() -> String {
    std::fs::read_to_string(profile_path()).unwrap()
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    profile_path();
    execute(parse(args), &ctx()).await
}

#[tokio::test]
async fn an_unknown_authz_mode_is_rejected() {
    let err = run(&["governance", "set", "--mode", "telepathy"])
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("telepathy"), "{err:#}");
}

#[tokio::test]
async fn webhook_mode_requires_a_url() {
    let err = run(&["governance", "set", "--mode", "webhook"])
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("requires --url"), "{err:#}");
}

#[tokio::test]
async fn webhook_mode_with_a_url_is_written_with_its_timeout() {
    run(&[
        "governance",
        "set",
        "--mode",
        "webhook",
        "--url",
        "https://cov-authz.example/decide",
        "--timeout-ms",
        "750",
    ])
    .await
    .unwrap();

    let yaml = profile_yaml();
    assert!(yaml.contains("cov-authz.example"), "{yaml}");
    assert!(yaml.contains("750"), "{yaml}");

    run(&["governance", "show"]).await.unwrap();
}

#[tokio::test]
async fn unrestricted_mode_requires_the_exact_acknowledgement() {
    let missing = run(&["governance", "set", "--mode", "unrestricted"])
        .await
        .unwrap_err();
    assert!(
        format!("{missing:#}").contains("requires --acknowledgement"),
        "{missing:#}"
    );

    let wrong = run(&[
        "governance",
        "set",
        "--mode",
        "unrestricted",
        "--acknowledgement",
        "sure whatever",
    ])
    .await
    .unwrap_err();
    assert!(
        format!("{wrong:#}").contains("requires --acknowledgement"),
        "{wrong:#}"
    );
}

#[tokio::test]
async fn unrestricted_mode_with_the_exact_acknowledgement_is_written() {
    run(&[
        "governance",
        "set",
        "--mode",
        "unrestricted",
        "--acknowledgement",
        UNRESTRICTED_ACK,
    ])
    .await
    .unwrap();

    assert!(
        profile_yaml().contains("unrestricted"),
        "{}",
        profile_yaml()
    );
}

#[tokio::test]
async fn extension_and_disabled_modes_need_no_extra_arguments() {
    run(&["governance", "set", "--mode", "extension"])
        .await
        .unwrap();
    assert!(profile_yaml().contains("extension"), "{}", profile_yaml());

    run(&["governance", "set", "--mode", "DISABLED"])
        .await
        .unwrap();
    assert!(profile_yaml().contains("disabled"), "{}", profile_yaml());
}
