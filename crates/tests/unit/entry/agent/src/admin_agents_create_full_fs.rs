//! Tests for `admin agents create` with the full optional-argument surface.
//!
//! Each optional field has its own resolver; supplying them all exercises the
//! non-prompted path end to end and proves the generated YAML carries them.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_agent_entry::cli::agents::execute;

use crate::common::{agents_host, json_cli, parse_agents, with_boot};

fn services_root() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let root = boot.services_path.clone();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(root.join("config/config.yaml"), "mcp_servers: {}\n").unwrap();
    root
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    let ctx = with_boot(json_cli());
    execute(parse_agents(args), &agents_host(&ctx))
        .await
        .map(drop)
}

#[tokio::test]
async fn every_optional_field_reaches_the_generated_definition() {
    let root = services_root();

    run(&[
        "create",
        "--name",
        "covfull",
        "--port",
        "9401",
        "--endpoint",
        "/api/v1/agents/covfull/",
        "--display-name",
        "Coverage Full",
        "--description",
        "An agent with every field supplied",
        "--version",
        "3.1.4",
        "--icon-url",
        "https://example.com/icon.png",
        "--documentation-url",
        "https://example.com/docs",
        "--provider",
        "anthropic",
        "--model",
        "claude-sonnet-4-5",
        "--system-prompt",
        "You are fully specified.",
        "--streaming",
        "true",
        "--push-notifications",
        "false",
        "--extended-agent-card",
        "true",
        "--enabled",
    ])
    .await
    .unwrap();

    let yaml = std::fs::read_to_string(root.join("agents/covfull.yaml")).unwrap();
    assert!(yaml.contains("covfull"), "{yaml}");
    assert!(yaml.contains("9401"), "{yaml}");
    assert!(yaml.contains("Coverage Full"), "{yaml}");
    assert!(yaml.contains("3.1.4"), "{yaml}");
    assert!(yaml.contains("https://example.com/icon.png"), "{yaml}");
    assert!(yaml.contains("You are fully specified."), "{yaml}");
    assert!(yaml.contains("extendedAgentCard: true"), "{yaml}");
}

#[tokio::test]
async fn a_system_prompt_file_is_read_from_disk() {
    let root = services_root();
    let tmp = tempfile::tempdir().unwrap();
    let prompt = tmp.path().join("prompt.md");
    std::fs::write(&prompt, "Prompt loaded from a file.").unwrap();

    run(&[
        "create",
        "--name",
        "covprompted",
        "--provider",
        "anthropic",
        "--port",
        "9402",
        "--display-name",
        "Coverage Prompted",
        "--description",
        "Reads its prompt from a file",
        "--system-prompt-file",
        prompt.to_str().unwrap(),
    ])
    .await
    .unwrap();

    let yaml = std::fs::read_to_string(root.join("agents/covprompted.yaml")).unwrap();
    assert!(yaml.contains("Prompt loaded from a file."), "{yaml}");
}

#[tokio::test]
async fn a_missing_system_prompt_file_is_reported() {
    services_root();

    let err = run(&[
        "create",
        "--name",
        "covmissingprompt",
        "--provider",
        "anthropic",
        "--port",
        "9403",
        "--display-name",
        "Coverage Missing",
        "--description",
        "Points at a file that is not there",
        "--system-prompt-file",
        "/nonexistent/prompt.md",
    ])
    .await
    .unwrap_err();

    assert!(format!("{err:#}").contains("prompt.md"), "{err:#}");
}

#[tokio::test]
async fn creating_the_same_agent_twice_is_refused() {
    services_root();

    let args = [
        "create",
        "--name",
        "covduplicate",
        "--provider",
        "anthropic",
        "--port",
        "9404",
        "--display-name",
        "Coverage Duplicate",
        "--description",
        "Created twice",
        "--system-prompt",
        "x",
    ];

    run(&args).await.unwrap();
    let err = run(&args).await.unwrap_err();
    assert!(format!("{err:#}").contains("covduplicate"), "{err:#}");
}
