//! Mixed MCP availability through the public agent-tools command.

use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::admin::agents::{self, AgentsCommands};
use systemprompt_cli::paths::ResolvedPaths;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_cloud::{CliSession, SessionBinding, SessionIdentity, SessionKey, SessionStore};
use systemprompt_database::CreateServiceInput;
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken};
use systemprompt_models::auth::UserType;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    DisposableDb, TestBootstrap, fixture_app_context_with, fixture_user_id,
    init_services_bootstrap, install_test_signing_key,
};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const AGENT: &str = "covmixedtools";
const HEALTHY: &str = "covhealthytools";
const STOPPED: &str = "covstoppedtools";
const HELPER: &str = "commands::agent_tools_mixed_lifecycle::mixed_agent_tools_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: AgentsCommands,
}

fn parse(args: &[&str]) -> AgentsCommands {
    Harness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .expect("parse agent-tools command")
        .command
}

fn services_yaml(healthy: &str, stopped: &str) -> String {
    format!(
        r#"agents:
  {AGENT}:
    name: {AGENT}
    port: 9503
    endpoint: /api/v1/agents/{AGENT}/
    enabled: true
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: {AGENT}
      displayName: Mixed Tools Agent
      description: Mixed MCP availability fixture
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
        include: [{HEALTHY}, {STOPPED}]
      skills:
        source: instance
      provider: anthropic
      model: claude-sonnet-4-5
      toolModelOverrides: {{}}
    oauth:
      required: false
      scopes: []
      audience: a2a
mcp_servers:
  {HEALTHY}:
    type: external
    endpoint: {healthy}/mcp
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
  {STOPPED}:
    type: external
    endpoint: {stopped}/mcp
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
"#
    )
}

fn seed_session(boot: &TestBootstrap) {
    let profile_dir = boot.profile_path.parent().expect("profile directory");
    let profile_name_text = profile_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("profile name");
    let profile_name = ProfileName::try_new(profile_name_text).expect("valid profile name");
    let session = CliSession::builder(
        SessionBinding::new(profile_name, "https://issuer.test".to_owned()),
        SessionToken::new("mixed-tools-session-token"),
        SessionId::generate(),
        ContextId::generate(),
        SessionIdentity::new(
            fixture_user_id(),
            Email::try_new("mixed-tools@example.invalid").expect("fixture email"),
            UserType::Admin,
        ),
    )
    .with_profile_path(&boot.profile_path)
    .build();
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let mut store = SessionStore::load_or_create(&sessions_dir).expect("session store");
    store.upsert_session(&SessionKey::Local, session);
    store.set_active_with_profile(&SessionKey::Local, profile_name_text);
    store.save(&sessions_dir).expect("persist fixture session");
}

async fn mount_healthy_protocol(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": format!("systemprompt-cli-{HEALTHY}"), "version": "1.0.0"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "mixed-tools-session")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0", "id": 0,
                    "result": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {},
                        "serverInfo": {"name": "mixed-tools-fixture", "version": "1.0.0"}
                    }
                })),
        )
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list",
            "params": {"_meta": {"progressToken": 0}}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0", "id": 1,
                    "result": {"tools": [{
                        "name": "lookup_record",
                        "description": "Look up one owned record",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"record_id": {"type": "string"}},
                            "required": ["record_id"]
                        },
                        "outputSchema": {
                            "type": "object",
                            "properties": {"found": {"type": "boolean"}},
                            "required": ["found"]
                        }
                    }]}
                })),
        )
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

#[tokio::test]
#[ignore = "re-executed by healthy_tools_survive_a_stopped_configured_server"]
async fn mixed_agent_tools_helper() {
    let healthy = MockServer::start().await;
    let stopped = MockServer::start().await;
    mount_healthy_protocol(&healthy).await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&stopped)
        .await;
    let boot = init_services_bootstrap(&services_yaml(&healthy.uri(), &stopped.uri()));
    install_test_signing_key();
    seed_session(&boot);
    let database = DisposableDb::installed("cli_agent_tools_mixed")
        .await
        .expect("private mixed-tools database");
    let pool = database.pool().await.expect("private mixed-tools pool");
    let paths = PathsConfig {
        system: boot.system_path.display().to_string(),
        services: boot.services_path.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: Some(boot.system_path.join("web").display().to_string()),
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    };
    let app = fixture_app_context_with(
        &pool,
        database.url(),
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .expect("mixed-tools app context");
    app.service_repository()
        .create_service(CreateServiceInput {
            name: HEALTHY,
            module_name: "mcp",
            status: "running",
            port: healthy.address().port(),
            binary_mtime: None,
        })
        .await
        .expect("register only the healthy MCP server as running");
    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json)
            .with_profile_override(Some(boot.profile_path.display().to_string())),
        EnvOverrides::default(),
        app,
    );
    println!("BEGIN_MIXED_TOOLS");
    agents::execute(
        parse(&["tools", AGENT, "--detailed", "--timeout", "5"]),
        &context,
    )
    .await
    .expect("healthy server keeps mixed tool listing successful");
    println!("END_MIXED_TOOLS");
    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("mixed-tools stdout");
    let stderr = tempfile::NamedTempFile::new().expect("mixed-tools stderr");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().expect("stdout writer")))
        .stderr(Stdio::from(stderr.reopen().expect("stderr writer")));
    let mut child = command.spawn().expect("spawn mixed-tools helper");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().expect("poll mixed-tools helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).expect("read stdout"),
                stderr: std::fs::read(stderr.path()).expect("read stderr"),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap mixed-tools helper");
            panic!(
                "mixed-tools helper timed out ({status})\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&std::fs::read(stdout.path()).unwrap()),
                String::from_utf8_lossy(&std::fs::read(stderr.path()).unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn healthy_tools_survive_a_stopped_configured_server() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "mixed-tools helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 mixed-tools output");
    let document = stdout
        .split_once("BEGIN_MIXED_TOOLS\n")
        .and_then(|(_, tail)| tail.split_once("\nEND_MIXED_TOOLS"))
        .map(|(json, _)| json)
        .expect("mixed-tools JSON markers");
    let artifact: serde_json::Value = serde_json::from_str(document).expect("mixed-tools JSON");
    assert_eq!(artifact["artifact_type"], "table", "{artifact}");
    let column_names = artifact["columns"]
        .as_array()
        .expect("tool columns")
        .iter()
        .map(|column| column["name"].as_str().expect("column name"))
        .collect::<Vec<_>>();
    assert_eq!(
        column_names,
        ["name", "server", "description", "parameters_count",]
    );
    let rows = artifact["items"].as_array().expect("tool rows");
    assert_eq!(rows.len(), 1, "{artifact}");
    assert_eq!(rows[0]["name"], "lookup_record", "{artifact}");
    assert_eq!(rows[0]["server"], HEALTHY, "{artifact}");
    assert_eq!(
        rows[0]["description"], "Look up one owned record",
        "{artifact}"
    );
    assert_eq!(rows[0]["parameters_count"], 1, "{artifact}");
    assert_eq!(
        rows[0]["input_schema"],
        serde_json::json!({
            "type": "object",
            "properties": {"record_id": {"type": "string"}},
            "required": ["record_id"]
        }),
        "{artifact}"
    );
    assert_eq!(
        rows[0]["output_schema"],
        serde_json::json!({
            "type": "object",
            "properties": {"found": {"type": "boolean"}},
            "required": ["found"]
        }),
        "{artifact}"
    );
}
