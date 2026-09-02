//! Shared fixtures for config-driven MCP tests.
//!
//! Builds the `mcp_servers` / `agents` YAML a test needs, hands it to the
//! bootstrap fixture *before* the process-wide inits run, and scripts a
//! wiremock MCP endpoint that answers the streamable-HTTP handshake plus
//! `tools/list` and `tools/call`.
//!
//! **Run this crate under `cargo nextest`, never `cargo test`.** Two pieces of
//! process-global state make one process per test mandatory, and both fail
//! quietly rather than loudly:
//!
//! * `ServicesBootstrap` parses the services tree once, during
//!   `TestBootstrap`'s init, into a `OnceLock` with no reset, and
//!   `ConfigLoader::load()` memoises the same parse per process. A config
//!   written to the bootstrap path *after* that init is therefore never seen —
//!   which is why `bootstrap_with_services` supplies the YAML up front instead
//!   of writing over the file afterwards.
//! * `ProfileBootstrap`'s `PROFILE` is a `OnceLock` with no reset. The first
//!   `TestBootstrap` in a process wins permanently, and every later fixture
//!   directory is unreachable no matter what is written into it.
//!
//! The second is why a `reload()` here would not be enough, and why
//! `--test-threads=1` is not enough either. Under `cargo test` the failures
//! read as ordinary assertion failures (`startup failure surfaces: 0`) rather
//! than as the fixture never having been seen — which is exactly how they get
//! misdiagnosed as real regressions.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{TestBootstrap, init_services_bootstrap};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

static BOOTSTRAP: std::sync::OnceLock<TestBootstrap> = std::sync::OnceLock::new();

pub fn bootstrap_with_services(yaml: &str) -> &'static TestBootstrap {
    BOOTSTRAP.get_or_init(|| init_services_bootstrap(yaml))
}

pub fn installed_bootstrap() -> &'static TestBootstrap {
    BOOTSTRAP
        .get()
        .expect("bootstrap_with_services must run before the bootstrap paths are read")
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

pub fn internal_server_block(name: &str, port: u16) -> String {
    format!(
        r"  {name}:
    server_type: internal
    binary: {name}-bin
    package: null
    port: {port}
    enabled: true
    display_in_web: true
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
",
    )
}

pub fn external_server_block_with_accessor(name: &str, endpoint: &str) -> String {
    format!(
        r"  {name}:
    server_type: external
    binary: {name}-bin
    package: null
    port: 0
    endpoint: {endpoint}
    enabled: true
    display_in_web: true
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
    external_auth:
      token_endpoint: /api/public/{name}/token
",
    )
}

pub fn register_internal_extension(bootstrap: &TestBootstrap, name: &str) {
    let ext_dir = bootstrap.system_path.join("extensions").join(name);
    std::fs::create_dir_all(&ext_dir).expect("create extension dir");
    std::fs::write(
        ext_dir.join("manifest.yaml"),
        format!(
            "extension:\n  type: mcp\n  name: {name}\n  binary: {name}-bin\n  description: \
             harness extension\n  enabled: true\n"
        ),
    )
    .expect("write extension manifest");
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
      mcpServers:
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
            ema: false,
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

// A minimal MCP endpoint: enough of the streamable-HTTP handshake for the
// startup health probe (initialize, initialized, tools/list) to succeed. The
// tool list must be non-empty — the probe reads an empty list as "service may
// require authentication" and never reports healthy.
pub const STUB_SERVER: &str = r#"import json, os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_DELETE(self):
        self.send_response(200)
        self.end_headers()

    def do_POST(self):
        length = int(self.headers.get('content-length', 0))
        body = json.loads(self.rfile.read(length) or b'{}')
        method = body.get('method', '')
        if method.startswith('notifications/'):
            self.send_response(202)
            self.end_headers()
            return
        if method == 'initialize':
            result = {
                'protocolVersion': '2025-03-26',
                'capabilities': {'tools': {}},
                'serverInfo': {'name': 'stub', 'version': '1.0.0'},
            }
        elif method == 'tools/list':
            result = {'tools': [{
                'name': 'echo',
                'description': 'echoes its input',
                'inputSchema': {'type': 'object', 'properties': {}},
            }]}
        else:
            result = {}
        payload = json.dumps(
            {'jsonrpc': '2.0', 'id': body.get('id', 0), 'result': result}
        ).encode()
        self.send_response(200)
        self.send_header('content-type', 'application/json')
        self.send_header('mcp-session-id', 'stub-session')
        self.send_header('content-length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

ThreadingHTTPServer(('127.0.0.1', int(os.environ['MCP_PORT'])), Handler).serve_forever()
"#;

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

// Installs `<name>-bin` in the bootstrap bin dir as a launcher for the stub
// server. The script is leaked deliberately: the spawned child outlives the
// helper's scope.
pub fn install_stub_binary(bootstrap: &TestBootstrap, name: &str) -> PathBuf {
    let dir = tempfile::tempdir().expect("stub dir");
    let script_path = dir.path().join("mcp_stub.py");
    std::fs::write(&script_path, STUB_SERVER).expect("write stub");

    let binary = bootstrap.bin_path.join(format!("{name}-bin"));
    write_executable(
        &binary,
        &format!("#!/bin/sh\nexec python3 {}\n", script_path.display()),
    );
    std::mem::forget(dir);
    binary
}
