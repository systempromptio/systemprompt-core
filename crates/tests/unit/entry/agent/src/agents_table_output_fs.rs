//! `admin agents` config readers in the default table mode, which renders
//! through the terminal renderer rather than short-circuiting to JSON, and
//! their failure when the services view holds no loadable agent config.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use systemprompt_agent_entry::cli::agents::execute;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::CliConfig;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::fixture_admitted;
use systemprompt_traits::BackgroundTasks;

use crate::common::{agents_host, json_cli, parse_agents, with_boot};

fn agent_yaml() -> &'static str {
    r"agents:
  covtable:
    name: covtable
    port: 9501
    endpoint: /api/v1/agents/covtable/
    enabled: true
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: covtable
      displayName: Coverage Table
      description: Fixture agent for table-output coverage
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
      defaultInputModes:
      - text/plain
      defaultOutputModes:
      - text/plain
    metadata:
      systemPrompt: You are a fixture agent.
      mcpServers:
        source: instance
      skills:
        source: instance
      provider: anthropic
      model: claude-sonnet-4-5
      toolModelOverrides: {}
    oauth:
      required: false
      scopes: []
      audience: a2a
"
}

fn seed_agent() {
    let root = systemprompt_test_fixtures::ensure_test_bootstrap()
        .services_path
        .clone();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(root.join("agents/covtable.yaml"), agent_yaml()).unwrap();
    std::fs::write(
        root.join("config/config.yaml"),
        "includes:\n  - ../agents/covtable.yaml\nmcp_servers: {}\n",
    )
    .unwrap();
}

#[tokio::test]
async fn agent_config_commands_render_through_the_terminal_renderer() {
    seed_agent();
    let ctx = with_boot(CliConfig::new().with_interactive(false));
    let host = agents_host(&ctx);

    for args in [
        vec!["list"],
        vec!["list", "--enabled"],
        vec!["show", "covtable"],
    ] {
        execute(parse_agents(&args), &host).await.unwrap();
    }

    let err = execute(parse_agents(&["validate"]), &host)
        .await
        .expect_err("the fixture agent names a provider absent from ai.providers");
    assert!(
        format!("{err:#}").contains("Agent validation failed"),
        "{err:#}"
    );
}

#[tokio::test]
async fn an_agent_config_that_fails_to_load_is_reported_by_every_reader() {
    let boot = systemprompt_test_fixtures::init_unloadable_services_bootstrap(
        "http://127.0.0.1",
        "agents:\n  covbroken:\n    name: covbroken\n    port: 42\nmcp_servers: {}\n",
    );
    let kernel = Kernel::detached(
        BackgroundTasks::new(),
        Dependencies::default().with(Arc::new(boot.app_paths.clone())),
    );
    let ctx = CommandContext::new(json_cli(), EnvOverrides::default())
        .with_boot(Some(boot.boot.clone()))
        .with_admitted(Some(fixture_admitted()))
        .with_kernel(Arc::new(kernel));
    let host = agents_host(&ctx);

    for argv in [vec!["list"], vec!["validate"], vec!["show", "covbroken"]] {
        let err = execute(parse_agents(&argv), &host).await.unwrap_err();
        assert!(!format!("{err:#}").is_empty(), "{argv:?}");
    }
}
