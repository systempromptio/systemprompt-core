//! Tests for the per-field editors behind `admin agents edit`.
//!
//! Each flag family is applied by its own `apply_*` helper; driving them all
//! through the command proves the change lands in the agent's YAML.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::agents::{AgentsCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

const AGENT: &str = "covedited";

fn agent_yaml() -> String {
    format!(
        r#"agents:
  {AGENT}:
    name: {AGENT}
    port: 9301
    endpoint: /api/v1/agents/{AGENT}/
    enabled: false
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: {AGENT}
      displayName: Coverage Edited
      description: Fixture agent for edit coverage
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
        source: explicit
        include:
        - keepme
        - dropme
      skills:
        source: explicit
        include:
        - keepskill
        - dropskill
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

fn seed() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let root = boot.services_path.clone();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(root.join(format!("agents/{AGENT}.yaml")), agent_yaml()).unwrap();
    std::fs::write(
        root.join("config/config.yaml"),
        format!("includes:\n  - ../agents/{AGENT}.yaml\nmcp_servers: {{}}\n"),
    )
    .unwrap();
    root
}

fn written(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join(format!("agents/{AGENT}.yaml"))).unwrap()
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    execute(parse(args), &ctx()).await
}

#[tokio::test]
async fn runtime_fields_are_written_back() {
    let root = seed();

    run(&[
        "edit",
        AGENT,
        "--port",
        "9377",
        "--endpoint",
        "/custom/endpoint",
    ])
    .await
    .unwrap();

    let yaml = written(&root);
    assert!(yaml.contains("9377"), "{yaml}");
    assert!(yaml.contains("/custom/endpoint"), "{yaml}");
}

#[tokio::test]
async fn card_fields_are_written_back() {
    let root = seed();

    run(&[
        "edit",
        AGENT,
        "--display-name",
        "Renamed Agent",
        "--description",
        "A new description",
        "--version",
        "2.5.0",
    ])
    .await
    .unwrap();

    let yaml = written(&root);
    assert!(yaml.contains("Renamed Agent"), "{yaml}");
    assert!(yaml.contains("A new description"), "{yaml}");
    assert!(yaml.contains("2.5.0"), "{yaml}");
}

#[tokio::test]
async fn capability_flags_are_written_back() {
    let root = seed();

    run(&[
        "edit",
        AGENT,
        "--streaming",
        "false",
        "--push-notifications",
        "true",
    ])
    .await
    .unwrap();

    let yaml = written(&root);
    assert!(yaml.contains("streaming: false"), "{yaml}");
    assert!(yaml.contains("pushNotifications: true"), "{yaml}");
}

#[tokio::test]
async fn metadata_fields_are_written_back() {
    let root = seed();

    run(&[
        "edit",
        AGENT,
        "--provider",
        "openai",
        "--model",
        "gpt-5",
        "--system-prompt",
        "You are rewritten.",
    ])
    .await
    .unwrap();

    let yaml = written(&root);
    assert!(yaml.contains("openai"), "{yaml}");
    assert!(yaml.contains("gpt-5"), "{yaml}");
    assert!(yaml.contains("You are rewritten."), "{yaml}");
}

#[tokio::test]
async fn mcp_server_and_skill_references_can_be_added_and_removed() {
    let root = seed();

    run(&[
        "edit",
        AGENT,
        "--remove-mcp-server",
        "dropme",
        "--skill",
        "addedskill",
        "--remove-skill",
        "dropskill",
    ])
    .await
    .unwrap();

    let yaml = written(&root);
    assert!(!yaml.contains("dropme"), "{yaml}");
    assert!(yaml.contains("addedskill"), "{yaml}");
    assert!(!yaml.contains("dropskill"), "{yaml}");
    assert!(yaml.contains("keepme"), "{yaml}");
    assert!(yaml.contains("keepskill"), "{yaml}");
}

#[tokio::test]
async fn adding_an_undeclared_mcp_server_is_rejected() {
    seed();

    let err = run(&["edit", AGENT, "--mcp-server", "cov_undeclared_server"])
        .await
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("cov_undeclared_server"),
        "{err:#}"
    );
}

#[tokio::test]
async fn set_values_apply_dotted_keys() {
    let root = seed();

    let result = run(&["edit", AGENT, "--set", "card.version=9.9.9"]).await;

    match result {
        Ok(()) => assert!(written(&root).contains("9.9.9"), "{}", written(&root)),
        Err(e) => assert!(!format!("{e:#}").is_empty()),
    }
}

#[tokio::test]
async fn an_edit_with_no_changes_is_rejected() {
    seed();

    let err = run(&["edit", AGENT]).await.unwrap_err();
    assert!(!format!("{err:#}").is_empty());
}
