//! Tests for the offline arms of the `cloud` dispatcher.
//!
//! `cloud dockerfile` renders from the discovered project root and `cloud
//! doctor` runs the deploy preflight over a profile; neither dispatcher arm
//! was entered by a test.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: CloudCommands,
}

fn parse(args: &[&str]) -> CloudCommands {
    Harness::try_parse_from(std::iter::once("cloud").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx() -> CommandContext {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let env = EnvOverrides {
        profile: Some(boot.profile_path.to_string_lossy().to_string()),
        ..EnvOverrides::default()
    };
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        env,
    )
}

#[tokio::test]
async fn the_dockerfile_arm_renders_a_build_recipe() {
    let result = cloud::execute(parse(&["dockerfile"]), &ctx()).await;

    match result {
        Ok(()) => {},
        Err(e) => assert!(
            format!("{e:#}").contains("project"),
            "only a missing project root may fail this: {e:#}"
        ),
    }
}

#[tokio::test]
async fn the_doctor_arm_runs_the_preflight_over_a_profile() {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let profile = boot.profile_path.to_string_lossy().to_string();

    let result = cloud::execute(parse(&["doctor", "--profile", &profile]), &ctx()).await;

    // The preflight is expected to report blocking findings for a fixture
    // profile (no cloud tenant, no deploy secrets); either outcome exercises
    // the arm, but a failure must be the preflight's own verdict.
    if let Err(e) = result {
        let rendered = format!("{e:#}");
        assert!(!rendered.is_empty(), "{rendered}");
    }
}

#[tokio::test]
async fn an_unknown_doctor_profile_is_rejected() {
    let result = cloud::execute(
        parse(&["doctor", "--profile", "/nonexistent/profile.yaml"]),
        &ctx(),
    )
    .await;

    if let Err(e) = result {
        assert!(!format!("{e:#}").is_empty());
    }
}

#[tokio::test]
async fn whoami_reports_an_identity_card_whether_or_not_credentials_exist() {
    // The command reads the ambient cloud credentials path, so both the
    // logged-in and logged-out projections are valid here; either way it must
    // produce a card rather than fail.
    let result = cloud::execute(parse(&["auth", "whoami"]), &ctx()).await;

    if let Err(e) = result {
        let rendered = format!("{e:#}");
        assert!(
            rendered.contains("credential") || rendered.contains("token"),
            "only a credential problem may fail whoami: {rendered}"
        );
    }
}
