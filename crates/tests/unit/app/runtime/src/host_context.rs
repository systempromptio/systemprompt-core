//! A spawned MCP server boots its `HostContext` from the child environment
//! alone: it never reads the profile's secrets source, holds no secret it was
//! not granted, refuses to start without the verification keys, and serves a
//! hop-authenticated tool call on what it was given.

use std::sync::{Arc, OnceLock};

use serde_json::{Value, json};
use systemprompt_identifiers::{McpServerId, headers};
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_runtime::mcp_host::serve_on;
use systemprompt_runtime::{HostBootError, HostContext};
use systemprompt_test_fixtures::{
    TestBootstrap, host_child_env, init_services_bootstrap, mint_mcp_hop,
};

use crate::documents_mcp_support::registered;

const SERVICES: &str = "mcp_servers:
  host_docs:
    server_type: internal
    binary: host-docs
    package: null
    port: 5396
    enabled: true
    display_in_web: false
    oauth:
      required: true
      scopes: [user]
      audience: mcp
      client_id: null
    toolsets:
      - id: documents
";

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn bootstrap() -> &'static TestBootstrap {
    BOOT.get_or_init(|| {
        let bootstrap = init_services_bootstrap(SERVICES);
        std::fs::write(
            bootstrap.services_path.join("config/files.yaml"),
            "files:\n  documents:\n    enabled: true\n",
        )
        .expect("write files.yaml");
        bootstrap
    })
}

async fn boot_with(
    env: std::collections::BTreeMap<String, String>,
) -> Result<HostContext, HostBootError> {
    HostContext::boot(move |name| env.get(name).cloned()).await
}

#[tokio::test]
async fn the_host_boots_without_ever_reading_the_profile_secrets_source() {
    let bootstrap = bootstrap();
    let profile = std::fs::read_to_string(&bootstrap.profile_path).expect("profile");
    let absent = bootstrap.profile_path.with_file_name("absent-secrets.json");
    assert!(!absent.exists());
    let file_secrets = bootstrap.profile_path.with_file_name("file-secrets.yaml");
    std::fs::write(
        &file_secrets,
        format!(
            "{profile}secrets:\n  source: file\n  secrets_path: {}\n",
            absent.display()
        ),
    )
    .expect("write profile");
    let mut env = host_child_env(bootstrap);
    env.insert(
        "SYSTEMPROMPT_PROFILE".to_owned(),
        file_secrets.display().to_string(),
    );

    let host = boot_with(env).await.expect("host boots on its env alone");

    assert!(!absent.exists(), "the secrets file was never created");
    assert!(host.granted_secrets().granted_names().is_empty());
    let secrets = host.secrets();
    assert!(secrets.manifest_signing_secret_seed().is_err());
    assert!(secrets.encryption_master_key().is_err());
    assert!(secrets.secret("oauth_at_rest_pepper").is_none());
}

#[tokio::test]
async fn the_host_refuses_to_boot_without_the_verification_keys() {
    let mut env = host_child_env(bootstrap());
    env.remove(VERIFICATION_KEYS_ENV);
    let err = boot_with(env).await.expect_err("no keys");
    assert!(
        matches!(err, HostBootError::VerificationKeysMissing),
        "{err:?}"
    );

    let mut env = host_child_env(bootstrap());
    env.insert(VERIFICATION_KEYS_ENV.to_owned(), "{}".to_owned());
    let err = boot_with(env).await.expect_err("bad keys");
    assert!(
        matches!(err, HostBootError::VerificationKeysInvalid(_)),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_granted_custom_secret_is_the_only_secret_the_host_holds() {
    let mut env = host_child_env(bootstrap());
    env.insert(
        "SYSTEMPROMPT_CUSTOM_SECRETS".to_owned(),
        "WEBHOOK_KEY".to_owned(),
    );
    env.insert("WEBHOOK_KEY".to_owned(), "whk".to_owned());
    env.insert("OAUTH_AT_REST_PEPPER".to_owned(), "p".repeat(40));
    let host = boot_with(env).await.expect("host");
    assert_eq!(
        host.granted_secrets().granted_names(),
        vec!["webhook_key".to_owned()]
    );
    assert!(host.secrets().secret("oauth_at_rest_pepper").is_none());
}

fn mcp_request(body: &Value, hop: Option<&str>, session: Option<&str>) -> reqwest::RequestBuilder {
    let client = reqwest::Client::new();
    let mut builder = client
        .post(format!("http://{}/mcp", ADDR.get().expect("addr")))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("x-session-id", "s-host")
        .header("x-trace-id", "t-host")
        .header("x-agent-name", "agent-host")
        .body(body.to_string());
    if let Some(hop) = hop {
        builder = builder.header(headers::MCP_HOP, hop);
    }
    if let Some(session) = session {
        builder = builder.header("mcp-session-id", session);
    }
    builder
}

static ADDR: OnceLock<std::net::SocketAddr> = OnceLock::new();

#[tokio::test]
async fn the_host_serves_a_hop_authenticated_tool_call_on_its_grants_alone() {
    let host = Arc::new(boot_with(host_child_env(bootstrap())).await.expect("host"));
    let issuer = host.config().jwt_issuer.clone();
    let server = McpServerId::new("host_docs");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    ADDR.set(listener.local_addr().expect("addr"))
        .expect("addr once");
    let task = tokio::spawn(serve_on(Arc::clone(&host), server.clone(), listener));
    let user = registered("host-call").await;

    let anonymous = mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "host-test", "version": "0"}}}),
        None,
        None,
    )
    .send()
    .await
    .expect("answer");
    assert_eq!(anonymous.status(), reqwest::StatusCode::UNAUTHORIZED);

    let initialize = mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "host-test", "version": "0"}}}),
        Some(&mint_mcp_hop(&server, user.clone(), None, &issuer)),
        None,
    )
    .send()
    .await
    .expect("initialize");
    assert!(initialize.status().is_success(), "{}", initialize.status());
    let session = initialize
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .expect("session id")
        .to_owned();
    let hop = mint_mcp_hop(&server, user.clone(), Some(&session), &issuer);

    let initialized = mcp_request(
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        Some(&hop),
        Some(&session),
    )
    .send()
    .await
    .expect("initialized");
    assert!(
        initialized.status().is_success(),
        "{}",
        initialized.status()
    );

    let call = mcp_request(
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": "documents_list", "arguments": {}}}),
        Some(&hop),
        Some(&session),
    )
    .send()
    .await
    .expect("tools/call");
    assert!(call.status().is_success(), "{}", call.status());
    let body = call.text().await.expect("body");
    assert!(body.contains("\"result\""), "{body}");
    assert!(!body.contains("\"isError\":true"), "{body}");

    let foreign = mint_mcp_hop(
        &McpServerId::new("another-server"),
        user,
        Some(&session),
        &issuer,
    );
    let refused = mcp_request(
        &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "documents_list", "arguments": {}}}),
        Some(&foreign),
        Some(&session),
    )
    .send()
    .await
    .expect("refused call");
    assert_eq!(refused.status(), reqwest::StatusCode::UNAUTHORIZED);
    task.abort();
}
