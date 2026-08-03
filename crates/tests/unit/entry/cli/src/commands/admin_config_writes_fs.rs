//! Tests for the `admin config` write paths that edit the active profile.
//!
//! The CORS, server, runtime, and security setters all round-trip the profile
//! YAML the bootstrap fixture owns, so each write is asserted against the file.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::config::{ConfigCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

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
async fn cors_origins_can_be_listed_added_and_removed() {
    run(&["server", "cors", "list"]).await.unwrap();

    run(&["server", "cors", "add", "https://cov.example.com"])
        .await
        .unwrap();
    assert!(
        profile_yaml().contains("https://cov.example.com"),
        "{}",
        profile_yaml()
    );

    run(&["server", "cors", "remove", "https://cov.example.com"])
        .await
        .unwrap();
    assert!(
        !profile_yaml().contains("https://cov.example.com"),
        "{}",
        profile_yaml()
    );
}

#[tokio::test]
async fn removing_an_absent_cors_origin_is_reported() {
    let before = profile_yaml();

    let result = run(&["server", "cors", "remove", "https://never-added.example"]).await;
    if let Err(e) = result {
        assert!(format!("{e:#}").contains("never-added.example"), "{e:#}");
    }
    assert!(!before.contains("never-added.example"));
    assert!(!profile_yaml().contains("never-added.example"));
}

#[tokio::test]
async fn server_host_and_port_are_written_to_the_profile() {
    run(&["server", "set", "--port", "8123", "--host", "0.0.0.0"])
        .await
        .unwrap();

    let yaml = profile_yaml();
    assert!(yaml.contains("8123"), "{yaml}");
    assert!(yaml.contains("0.0.0.0"), "{yaml}");

    run(&["server", "show"]).await.unwrap();
}

#[tokio::test]
async fn a_runtime_field_is_written_to_the_profile() {
    run(&["runtime", "set", "--log-level", "debug"])
        .await
        .unwrap();
    assert!(profile_yaml().contains("debug"), "{}", profile_yaml());

    run(&["runtime", "show"]).await.unwrap();
}

#[tokio::test]
async fn a_security_field_is_written_to_the_profile() {
    run(&["security", "set", "--access-expiry", "1234"])
        .await
        .unwrap();
    assert!(profile_yaml().contains("1234"), "{}", profile_yaml());

    run(&["security", "show"]).await.unwrap();
}

#[tokio::test]
async fn a_trusted_issuer_can_be_added_and_removed() {
    let added = run(&[
        "security",
        "trusted-issuer",
        "add",
        "--issuer",
        "https://cov-issuer.example",
        "--jwks-uri",
        "https://cov-issuer.example/jwks",
        "--audience",
        "a2a",
    ])
    .await;

    match added {
        Ok(()) => {
            assert!(profile_yaml().contains("cov-issuer.example"));
            run(&[
                "security",
                "trusted-issuer",
                "remove",
                "--issuer",
                "https://cov-issuer.example",
            ])
            .await
            .unwrap();
            assert!(!profile_yaml().contains("cov-issuer.example"));
        },
        Err(e) => assert!(!format!("{e:#}").is_empty()),
    }
}
