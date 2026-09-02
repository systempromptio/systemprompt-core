//! Table-output runs of the command dispatchers.
//!
//! Every other dispatcher test in this suite uses `OutputFormat::Json`, which
//! short-circuits before the terminal renderer. Driving the same commands in
//! the default table mode is the only way into `render_terminal`'s per-artifact
//! arms and the post-render summaries commands print only for a terminal.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::agents::AgentsCommands;
use systemprompt_cli::admin::config::rate_limits::RateLimitsCommands;
use systemprompt_cli::core::CoreCommands;
use systemprompt_cli::plugins::PluginsCommands;
use systemprompt_cli::web::WebCommands;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, ScriptedPrompter};

const CONTENT_CONFIG: &str = "content_sources:\n  blog:\n    path: content/blog\n    source_id: \
                              blog\n    category_id: blog\n    enabled: true\n    sitemap:\n      \
                              enabled: true\n      url_pattern: /blog/{slug}\n      priority: \
                              0.7\n      changefreq: weekly\ncategories:\n  blog:\n    name: blog\n";

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
      toolModelOverrides: {}
    oauth:
      required: false
      scopes: []
      audience: a2a
"
}

fn services_root() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let root = boot.services_path.clone();

    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(root.join("agents/covtable.yaml"), agent_yaml()).unwrap();
    std::fs::write(
        root.join("config/config.yaml"),
        "includes:\n  - ../agents/covtable.yaml\nmcp_servers: {}\n",
    )
    .unwrap();
    std::fs::write(root.join("content/config.yaml"), CONTENT_CONFIG).unwrap();

    let skill = root.join("skills/covtableskill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: covtableskill\nname: Coverage Table Skill\ndescription: Fixture skill\n",
    )
    .unwrap();
    std::fs::write(skill.join("index.md"), "# Skill\n\nBody.\n").unwrap();

    let templates = root.join("web/templates");
    std::fs::create_dir_all(&templates).unwrap();
    std::fs::write(
        templates.join("templates.yaml"),
        "templates:\n  base:\n    content_types: [blog]\n",
    )
    .unwrap();
    std::fs::write(templates.join("base.html"), "<html></html>").unwrap();

    let assets = root.join("web/assets");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("logo.svg"), "<svg/>").unwrap();

    systemprompt_test_fixtures::refresh_services_config();
    root
}

fn table_config() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn table_ctx() -> CommandContext {
    CommandContext::new(table_config(), EnvOverrides::default())
}

fn prompter() -> ScriptedPrompter {
    ScriptedPrompter::new(Vec::<String>::new())
}

#[derive(Debug, Parser)]
struct WebHarness {
    #[command(subcommand)]
    cmd: WebCommands,
}

#[derive(Debug, Parser)]
struct PluginsHarness {
    #[command(subcommand)]
    cmd: PluginsCommands,
}

#[derive(Debug, Parser)]
struct CoreHarness {
    #[command(subcommand)]
    cmd: CoreCommands,
}

#[derive(Debug, Parser)]
struct AgentsHarness {
    #[command(subcommand)]
    cmd: AgentsCommands,
}

#[derive(Debug, Parser)]
struct RateLimitsHarness {
    #[command(subcommand)]
    cmd: RateLimitsCommands,
}

#[tokio::test]
async fn web_commands_render_through_the_terminal_renderer() {
    services_root();
    let ctx = table_ctx();

    for args in [
        vec!["content-types", "list"],
        vec!["content-types", "show", "blog"],
        vec!["templates", "list"],
        vec!["templates", "show", "base"],
        vec!["assets", "list"],
        vec!["assets", "show", "logo.svg"],
        vec!["sitemap", "show"],
    ] {
        let cmd = WebHarness::try_parse_from(std::iter::once("web").chain(args.iter().copied()))
            .unwrap()
            .cmd;
        systemprompt_cli::web::execute(cmd, &ctx).unwrap();
    }
}

#[tokio::test]
async fn plugin_commands_render_through_the_terminal_renderer() {
    let ctx = table_ctx();

    for args in [
        vec!["list"],
        vec!["config"],
        vec!["capabilities"],
        vec!["validate"],
    ] {
        let cmd =
            PluginsHarness::try_parse_from(std::iter::once("plugins").chain(args.iter().copied()))
                .unwrap()
                .cmd;
        systemprompt_cli::plugins::execute(cmd, &ctx).await.unwrap();
    }
}

#[tokio::test]
async fn core_commands_render_through_the_terminal_renderer() {
    services_root();
    let ctx = table_ctx();

    for args in [
        vec!["skills", "list"],
        vec!["skills", "show", "covtableskill"],
        vec!["plugins", "list"],
        vec!["plugins", "validate"],
        vec!["hooks", "list"],
        vec!["hooks", "validate"],
    ] {
        let cmd = CoreHarness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
            .unwrap()
            .cmd;
        systemprompt_cli::core::execute(cmd, &ctx).await.unwrap();
    }
}

#[tokio::test]
async fn agent_config_commands_render_through_the_terminal_renderer() {
    services_root();
    let ctx = table_ctx();

    for args in [
        vec!["list"],
        vec!["list", "--enabled"],
        vec!["show", "covtable"],
        vec!["validate"],
    ] {
        let cmd =
            AgentsHarness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
                .unwrap()
                .cmd;
        systemprompt_cli::admin::agents::execute(cmd, &ctx)
            .await
            .unwrap();
    }
}

#[test]
fn rate_limit_reports_render_through_the_terminal_renderer() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let prompter = prompter();
    let config = table_config();

    for args in [
        vec!["show"],
        vec!["validate"],
        vec!["preset", "list"],
        vec!["preset", "show", "production"],
    ] {
        let cmd = RateLimitsHarness::try_parse_from(
            std::iter::once("rate-limits").chain(args.iter().copied()),
        )
        .unwrap()
        .cmd;
        systemprompt_cli::admin::config::rate_limits::execute(cmd, &prompter, &config).unwrap();
    }
}
