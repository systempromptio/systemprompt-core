//! A spawned MCP server boots its `HostContext` from the child environment
//! alone: it never reads the profile's secrets source, holds no secret it was
//! not granted, refuses to start without the verification keys, and serves a
//! hop-authenticated tool call on what it was given, presented with the hop and
//! headers the API proxy mints for the request. It runs extension seam
//! discovery as the API does, over a context whose token authority verifies
//! and never signs.

use std::sync::{Arc, OnceLock};

use serde_json::{Value, json};
use systemprompt_api::services::proxy::service::hop::{HopGrant, hop_identity};
use systemprompt_api::services::proxy::service::outbound::managed_mcp_headers;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, McpServerId, ServiceName, SessionId, TraceId,
};
use systemprompt_mcp_host::toolsets::serve_on;
use systemprompt_mcp_host::{HostBootError, HostContext};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, TokenAuthority as _};
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_test_fixtures::{
    TestBootstrap, host_child_env, init_services_bootstrap, mint_mcp_hop,
    seed_fixture_system_admin, test_db_pool, test_token_authority,
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
    seed_fixture_system_admin(&test_db_pool().await)
        .await
        .expect("seed the system admin");
    HostContext::boot(
        systemprompt_extension::artifact_ui::HostRole::McpServer,
        move |name| env.get(name).cloned(),
        crate::boot::host_admit(),
    )
    .await
}

#[tokio::test]
async fn the_host_boots_without_ever_reading_the_profile_secrets_source() {
    let bootstrap = bootstrap();
    let profile = std::fs::read_to_string(&bootstrap.profile_path)
        .expect("profile")
        .replace(systemprompt_test_fixtures::FIXTURE_SECRETS_BLOCK, "");
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

fn edge_headers(
    server: &McpServerId,
    user: &AuthenticatedUser,
    mcp_session: Option<&str>,
    issuer: &str,
) -> http::HeaderMap {
    let context = RequestContext::new(
        SessionId::new("s-host"),
        TraceId::new("t-host"),
        ContextId::generate(),
        AgentName::new("agent-host"),
        Actor::user(user.id.clone()),
    )
    .with_user(user.clone());
    let mut incoming = http::HeaderMap::new();
    if let Some(session) = mcp_session {
        incoming.insert("mcp-session-id", session.parse().expect("session header"));
    }
    let identity = hop_identity(&HopGrant {
        service_name: &ServiceName::new(server.as_str()),
        context: &context,
        user: Some(user),
        request_headers: &incoming,
    });
    let hop = test_token_authority(issuer)
        .mint_mcp_hop(server, &identity)
        .expect("mint the hop the proxy mints");
    managed_mcp_headers(
        &incoming,
        &context,
        &http::HeaderValue::from_str(&hop).expect("hop header"),
    )
}

fn mcp_request(body: &Value, edge: Option<http::HeaderMap>) -> reqwest::RequestBuilder {
    let client = reqwest::Client::new();
    let mut builder = client
        .post(format!("http://{}/mcp", ADDR.get().expect("addr")))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("x-session-id", "s-host")
        .header("x-trace-id", "t-host")
        .header("x-agent-name", "agent-host")
        .body(body.to_string());
    if let Some(edge) = edge {
        builder = builder.headers(edge);
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
    )
    .send()
    .await
    .expect("answer");
    assert_eq!(anonymous.status(), reqwest::StatusCode::UNAUTHORIZED);

    let initialize = mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "host-test", "version": "0"}}}),
        Some(edge_headers(&server, &user, None, &issuer)),
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

    let initialized = mcp_request(
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        Some(edge_headers(&server, &user, Some(&session), &issuer)),
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
        Some(edge_headers(&server, &user, Some(&session), &issuer)),
    )
    .send()
    .await
    .expect("tools/call");
    assert!(call.status().is_success(), "{}", call.status());
    let body = call.text().await.expect("body");
    assert!(body.contains("\"result\""), "{body}");
    assert!(!body.contains("\"isError\":true"), "{body}");

    let foreign = edge_headers(
        &McpServerId::new("another-server"),
        &user,
        Some(&session),
        &issuer,
    );
    let refused = mcp_request(
        &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "documents_list", "arguments": {}}}),
        Some(foreign),
    )
    .send()
    .await
    .expect("refused call");
    assert_eq!(refused.status(), reqwest::StatusCode::UNAUTHORIZED);
    task.abort();
}

#[tokio::test]
async fn the_token_authority_the_host_holds_verifies_and_never_signs() {
    use systemprompt_models::auth::{McpHopIdentity, TokenError, UserType};

    let host = boot_with(host_child_env(bootstrap())).await.expect("host");
    let tokens = std::sync::Arc::clone(&host.parts().tokens);
    let server = McpServerId::new("host_docs");
    let user = registered("host-ext-ctx").await;
    let identity = McpHopIdentity {
        user_id: user.id.clone(),
        user: Some(user.clone()),
        session_id: systemprompt_identifiers::SessionId::new("s-ext"),
        user_type: UserType::User,
        client_id: None,
        act_chain: Vec::new(),
        purpose: None,
        interaction: None,
        mcp_session: None,
        execution: None,
    };
    let refused = tokens
        .mint_mcp_hop(&server, &identity)
        .expect_err("the host holds no signing key");
    assert!(matches!(refused, TokenError::Signing(_)), "{refused:?}");
    assert!(matches!(
        tokens.sign_claims(&json!({"exp": 1})),
        Err(TokenError::Signing(_))
    ));

    let hop = mint_mcp_hop(&server, user.clone(), None, &host.config().jwt_issuer);
    let claims = tokens
        .verify_mcp_hop(&hop, &server)
        .expect("a hop the API signed verifies against the public keys");
    assert_eq!(claims.identity.user_id, user.id);
    assert!(
        host.state()
            .get::<std::sync::Arc<systemprompt_oauth::repository::OAuthRepositories>>()
            .is_err()
    );
}
