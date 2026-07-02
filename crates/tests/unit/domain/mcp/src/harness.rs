//! Shared fixtures for config-driven MCP tests.
//!
//! Writes `mcp_servers` / `agents` entries into the bootstrap services config
//! (re-read on every `ConfigLoader::load()`), and scripts a wiremock MCP
//! endpoint that answers the streamable-HTTP handshake plus `tools/list` and
//! `tools/call`.

use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::TestBootstrap;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub fn write_services_config(bootstrap: &TestBootstrap, yaml: &str) {
    std::fs::write(bootstrap.services_path.join("config/config.yaml"), yaml)
        .expect("write services config");
}

pub struct ExternalServerSpec<'a> {
    pub name: &'a str,
    pub endpoint: &'a str,
    pub oauth_required: bool,
    pub enabled: bool,
}

pub fn external_server_block(spec: &ExternalServerSpec<'_>) -> String {
    format!(
        r"  {name}:
    server_type: external
    binary: {name}-bin
    package: null
    port: 0
    endpoint: {endpoint}
    enabled: {enabled}
    display_in_web: true
    oauth:
      required: {oauth}
      scopes: []
      audience: mcp
      client_id: null
",
        name = spec.name,
        endpoint = spec.endpoint,
        enabled = spec.enabled,
        oauth = spec.oauth_required,
    )
}

pub fn config_with_servers(server_blocks: &[String]) -> String {
    format!("mcp_servers:\n{}", server_blocks.join(""))
}

pub fn agent_block(agent: &str, servers: &[&str]) -> String {
    let include = servers
        .iter()
        .map(|s| format!("          - {s}\n"))
        .collect::<String>();
    format!(
        r#"agents:
  {agent}:
    name: {agent}
    port: 9251
    endpoint: http://127.0.0.1:9251
    enabled: true
    card:
      protocolVersion: "0.3.0"
      displayName: Harness Agent
      description: Agent used by MCP harness tests.
      version: "1.0.0"
    metadata:
      mcp_servers:
        include:
{include}    oauth:
      required: false
"#
    )
}

pub fn request_context(tag: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("s-{tag}")),
        TraceId::new(format!("t-{tag}")),
        ContextId::generate(),
        AgentName::new(format!("agent-{tag}")),
    )
    .with_actor(Actor::user(UserId::new(format!("user-{tag}"))))
}

pub async fn mount_mcp_endpoint(server: &MockServer, tools: serde_json::Value) {
    let initialize_result = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0,
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "scripted", "version": "1.0.0"}
        }
    });

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "initialize"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-harness")
                .set_body_json(initialize_result),
        )
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/list"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"tools": tools}
                })),
        )
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "content": [{"type": "text", "text": "harness output"}],
                        "isError": false
                    }
                })),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(405))
        .mount(server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

pub fn internal_mcp_config(name: &str, port: u16) -> systemprompt_models::mcp::McpServerConfig {
    use systemprompt_models::auth::JwtAudience;
    use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};

    systemprompt_models::mcp::McpServerConfig {
        name: name.to_owned(),
        owner: systemprompt_test_fixtures::fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: format!("{name}-bin"),
        enabled: true,
        display_in_web: true,
        port,
        crate_path: std::path::PathBuf::from("."),
        display_name: format!("{name} Server"),
        description: format!("{name} MCP Server"),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: std::collections::HashMap::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.1.0".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: std::collections::HashMap::default(),
    }
}

pub fn external_mcp_config(
    name: &str,
    endpoint: &str,
) -> systemprompt_models::mcp::McpServerConfig {
    use systemprompt_models::mcp::deployment::McpServerType;

    let mut config = internal_mcp_config(name, 0);
    config.server_type = McpServerType::External;
    config.binary = String::new();
    config.crate_path = std::path::PathBuf::new();
    config.remote_endpoint = endpoint.to_owned();
    config
}

pub fn default_tools_json() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "echo",
            "description": "Echo a message",
            "inputSchema": {"type": "object", "properties": {"message": {"type": "string"}}}
        },
        {
            "name": "shout",
            "inputSchema": {"type": "object"},
            "outputSchema": {"type": "object"}
        }
    ])
}
