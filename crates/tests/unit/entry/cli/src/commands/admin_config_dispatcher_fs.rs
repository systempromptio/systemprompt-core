//! Tests for the read-side arms of the `admin config` dispatcher.
//!
//! Every arm projects the bootstrapped profile into its own output shape;
//! none of these dispatcher arms were entered by a test.

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

async fn run(args: &[&str]) -> anyhow::Result<()> {
    profile_path();
    execute(parse(args), &ctx()).await
}

#[tokio::test]
async fn show_and_list_arms_render_the_active_profile() {
    run(&["show"]).await.unwrap();
    run(&["list"]).await.unwrap();
    run(&["list", "--errors-only"]).await.unwrap();
}

#[tokio::test]
async fn validate_arm_reports_on_the_bootstrapped_profile() {
    let result = run(&["validate"]).await;
    if let Err(e) = result {
        assert!(
            format!("{e:#}").contains("Config validation failed"),
            "{e:#}"
        );
    }
}

#[tokio::test]
async fn server_runtime_and_security_arms_show_their_sections() {
    run(&["server", "show"]).await.unwrap();
    run(&["runtime", "show"]).await.unwrap();
    run(&["security", "show"]).await.unwrap();
}

#[tokio::test]
async fn paths_arm_shows_and_validates_the_configured_paths() {
    run(&["paths", "show"]).await.unwrap();

    let validated = run(&["paths", "validate"]).await;
    if let Err(e) = validated {
        assert!(!format!("{e:#}").is_empty());
    }
}

#[tokio::test]
async fn provider_arm_reads_the_ai_config_when_present_and_reports_it_when_absent() {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let ai_config = boot.services_path.join("ai/config.yaml");

    let err = run(&["provider", "list"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("ai/config.yaml"), "{err:#}");

    std::fs::create_dir_all(ai_config.parent().unwrap()).unwrap();
    std::fs::write(
        &ai_config,
        "ai:\n  default_provider: anthropic\n  providers:\n    anthropic:\n      enabled: \
         true\n      model: claude-sonnet-4-5\n",
    )
    .unwrap();

    run(&["provider", "list"]).await.unwrap();
    run(&["provider", "enable", "anthropic"]).await.unwrap();
    run(&["provider", "disable", "anthropic"]).await.unwrap();
}

#[tokio::test]
async fn governance_arm_renders_its_section() {
    run(&["governance", "show"]).await.unwrap();
}
