//! Profile-backed tests for the `admin agents` config commands.
//!
//! `list`, `show`, `validate`, `create`, and `edit` all read (and write) the
//! services config the bootstrapped profile points at, so the fixture tree is
//! seeded per test and the create/edit paths asserted against the files they
//! produce.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::agents::{AgentsCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

fn agent_yaml(name: &str, port: u16, display: &str, enabled: bool) -> String {
    format!(
        r#"agents:
  {name}:
    name: {name}
    port: {port}
    endpoint: /api/v1/agents/{name}/
    enabled: {enabled}
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: {name}
      displayName: {display}
      description: Fixture agent for coverage
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: true
      defaultInputModes:
      - text/plain
      defaultOutputModes:
      - text/plain
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: You are a fixture agent.
      mcpServers:
        source: instance
      skills:
        source: instance
      provider: anthropic
      model: claude-sonnet-4-5
      toolModelOverrides: {{}}
    oauth:
      required: false
      scopes: []
      audience: a2a
"#
    )
}


#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: AgentsCommands,
}

fn parse(args: &[&str]) -> AgentsCommands {
    Harness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
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

fn services_root() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    boot.services_path.clone()
}

fn seed_agents() -> PathBuf {
    let root = services_root();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(
        root.join("agents/covlister.yaml"),
        agent_yaml("covlister", 9101, "Coverage Lister", true),
    )
    .unwrap();
    std::fs::write(
        root.join("agents/covdormant.yaml"),
        agent_yaml("covdormant", 9102, "Coverage Dormant", false),
    )
    .unwrap();
    std::fs::write(
        root.join("config/config.yaml"),
        "includes:\n  - ../agents/covlister.yaml\n  - ../agents/covdormant.yaml\nmcp_servers: {}\n",
    )
    .unwrap();
    root
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    execute(parse(args), &ctx()).await
}

#[tokio::test]
async fn list_renders_all_enabled_and_disabled_agents() {
    seed_agents();

    run(&["list"]).await.unwrap();
    run(&["list", "--enabled"]).await.unwrap();
    run(&["list", "--disabled"]).await.unwrap();
}

#[tokio::test]
async fn list_with_a_name_shows_that_agent_and_rejects_unknown_ones() {
    seed_agents();

    run(&["list", "covlister"]).await.unwrap();

    let err = run(&["list", "ghost"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("'ghost' not found"));
}

#[tokio::test]
async fn show_renders_a_seeded_agent_and_rejects_unknown_ones() {
    seed_agents();

    run(&["show", "covdormant"]).await.unwrap();

    let err = run(&["show", "ghost"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("ghost"));
}

#[tokio::test]
async fn validate_accepts_the_seeded_configuration() {
    seed_agents();

    run(&["validate"]).await.unwrap();
}

#[tokio::test]
async fn create_writes_a_new_agent_definition() {
    let root = seed_agents();

    run(&[
        "create",
        "--name",
        "covcreated",
        "--port",
        "9133",
        "--display-name",
        "Coverage Created",
        "--description",
        "Created by the coverage suite",
        "--system-prompt",
        "You are created.",
        "--enabled",
    ])
    .await
    .unwrap();

    let written = std::fs::read_to_string(root.join("agents/covcreated.yaml")).unwrap();
    assert!(written.contains("covcreated"));
    assert!(written.contains("9133"));
}

#[tokio::test]
async fn create_rejects_an_invalid_agent_name() {
    seed_agents();

    let err = run(&[
        "create",
        "--name",
        "Bad Name!",
        "--port",
        "9134",
        "--display-name",
        "Bad",
        "--description",
        "Bad",
        "--system-prompt",
        "Bad",
    ])
    .await
    .unwrap_err();

    assert!(!format!("{err:#}").is_empty());
}

#[tokio::test]
async fn create_requires_a_name_in_non_interactive_mode() {
    seed_agents();

    let err = run(&["create", "--port", "9135"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("--name is required"));
}

#[tokio::test]
async fn edit_toggles_the_enabled_flag_of_a_seeded_agent() {
    let root = seed_agents();

    run(&["edit", "covdormant", "--enable"]).await.unwrap();

    let written = std::fs::read_to_string(root.join("agents/covdormant.yaml")).unwrap();
    assert!(written.contains("enabled: true"), "{written}");
}

#[tokio::test]
async fn edit_rejects_an_unknown_agent() {
    seed_agents();

    let err = run(&["edit", "ghost", "--enable"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("ghost"));
}
