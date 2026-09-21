#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::admin::agents::{self, AgentsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_test_fixtures::init_services_bootstrap;

const AGENT: &str = "covemptytools";
const HELPER: &str = "commands::admin_agent_tools_empty_output::empty_agent_tools_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: AgentsCommands,
}

fn parse(args: &[&str]) -> AgentsCommands {
    Harness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .expect("parse agent tools command")
        .command
}

fn services_yaml() -> String {
    format!(
        r#"agents:
  {AGENT}:
    name: {AGENT}
    port: 9502
    endpoint: /api/v1/agents/{AGENT}/
    enabled: true
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: {AGENT}
      displayName: Empty Tools Agent
      description: Agent with no configured MCP servers
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: true
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: You are a fixture agent.
      mcpServers:
        source: explicit
        include: []
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

#[tokio::test]
#[ignore = "re-executed by agent_with_no_mcp_servers_returns_an_exact_empty_inventory"]
async fn empty_agent_tools_helper() {
    let _boot = init_services_bootstrap(&services_yaml());
    let context = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    );
    println!("BEGIN_EMPTY_TOOLS");
    agents::execute(parse(&["tools", AGENT]), &context)
        .await
        .expect("render empty tool inventory");
    println!("END_EMPTY_TOOLS");
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("tools stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("tools stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn agent-tools helper");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().expect("poll agent-tools helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap agent-tools helper");
            panic!(
                "agent-tools helper timed out ({status})\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&std::fs::read(stdout.path()).unwrap()),
                String::from_utf8_lossy(&std::fs::read(stderr.path()).unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn section<'a>(artifact: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    artifact["sections"]
        .as_array()
        .expect("presentation card sections")
        .iter()
        .find(|section| section["heading"] == name)
        .and_then(|section| section.get("content"))
        .unwrap_or_else(|| panic!("missing {name} section in {artifact}"))
}

#[test]
fn agent_with_no_mcp_servers_returns_an_exact_empty_inventory() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "agent-tools helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 agent-tools output");
    let json = stdout
        .split_once("BEGIN_EMPTY_TOOLS")
        .and_then(|(_, tail)| tail.split_once("END_EMPTY_TOOLS"))
        .map(|(json, _)| json.trim())
        .unwrap_or_else(|| panic!("missing agent-tools output markers in {stdout}"));
    let artifact: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|error| panic!("invalid JSON: {error}: {json}"));
    assert_eq!(
        artifact["title"],
        format!("Agent Tools: {AGENT} (no MCP servers configured)")
    );
    assert_eq!(section(&artifact, "agent"), AGENT);
    assert_eq!(section(&artifact, "tools"), &serde_json::json!([]));
    assert_eq!(
        section(&artifact, "summary"),
        &serde_json::json!({
            "total_tools": 0,
            "configured_servers": 0,
            "available_servers": 0
        })
    );
    assert!(
        artifact["sections"]
            .as_array()
            .unwrap()
            .iter()
            .all(|section| section["heading"] != "unavailable_servers"),
        "empty optional unavailable_servers must be omitted: {artifact}"
    );
}
