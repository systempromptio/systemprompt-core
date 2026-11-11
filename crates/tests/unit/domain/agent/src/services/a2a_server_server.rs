//! `a2a_server::Server::new` — what it refuses before building anything.
//!
//! This file was deliberately kept out of the coverage ignore list on the
//! grounds that it has real seams, and then sat at 19.6%. `run` binds a
//! listener and stays out of reach; `new` and `create_router` do not. The
//! suite boots a services tree carrying an `agents:` section so the registry
//! lookup succeeds and the router can actually be built and driven.

use std::sync::Arc;
use systemprompt_identifiers::AgentName;

use axum::body::Body;
use axum::http::Request;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::Server;
use systemprompt_models::ai::AiProvider;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_config, init_services_bootstrap, test_db_pool,
};
use systemprompt_test_mocks::MockAiProvider;
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};
use tower::ServiceExt;

struct StubJwt;

impl JwtValidationProvider for StubJwt {
    fn validate_token(&self, _token: &str) -> JwtResult<AgentJwtClaims> {
        Err(JwtProviderError::InvalidToken)
    }
    fn generate_token(&self, _params: GenerateTokenParams) -> JwtResult<String> {
        Ok("tok".to_owned())
    }
    fn generate_secure_token(&self, prefix: &str) -> String {
        format!("{prefix}-fake")
    }
}

const GATEWAY_YAML: &str = r#"
credentials:
  anthropic:
    kind: platform
    secret: anthropic_api_key
providers:
  anthropic:
    credential: anthropic
    endpoints:
      api:
        wire: anthropic
        url: http://127.0.0.1:1
models:
  claude-fixture-1:
    clients:
    - claude
    pricing:
      input_per_million: 3.0
      output_per_million: 15.0
      cache_read_per_million: 0.0
deployments:
  claude-fixture-1:
  - anthropic/api
"#;

const SERVICES_YAML: &str = r#"agents:
  a2a_fixture_agent:
    name: a2a_fixture_agent
    port: 9451
    endpoint: http://127.0.0.1:9451
    enabled: true
    card:
      protocolVersion: "0.3.0"
      displayName: A2A Fixture Agent
      description: Agent used to build a router in tests.
      version: "1.0.0"
    metadata: {}
    oauth:
      required: false
"#;

static BOOT: std::sync::OnceLock<TestBootstrap> = std::sync::OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_services_bootstrap(&format!("{SERVICES_YAML}{GATEWAY_YAML}")))
}

async fn state() -> Arc<AgentState> {
    let b = boot();
    let pool = test_db_pool().await;
    let config = Arc::new(fixture_config(&b.database_url));
    let repos = systemprompt_test_fixtures::a2a_repositories(&pool);
    let state = AgentState::new(
        pool.clone(),
        config,
        systemprompt_agent::AgentAuth {
            jwt_provider: Arc::new(StubJwt) as systemprompt_traits::DynJwtValidationProvider,
            signing_keys: systemprompt_test_fixtures::test_signing_keys(),
        },
        Arc::new(repos),
        systemprompt_test_mocks::recording_webhooks(),
        b.services.view(),
    );
    Arc::new(state)
}

fn ai() -> Arc<dyn AiProvider> {
    Arc::new(MockAiProvider::builder().build())
}

// Why: an unregistered name must fail rather than fall back to a default. A
// server that quietly served some other agent's card would answer for an
// identity nobody asked it to hold.
#[tokio::test]
async fn a_server_for_an_agent_that_is_not_registered_is_refused() {
    let agent_state = state().await;

    let err = Server::new(
        agent_state,
        ai(),
        &AgentName::new("agent-that-does-not-exist"),
        0,
    )
    .expect_err("an unregistered agent must not produce a server");

    assert!(
        format!("{err}").contains("agent-that-does-not-exist"),
        "the refusal should name the agent it could not find: {err}"
    );
}

// Why: the router is what decides which paths exist. A path answering anything
// but 404 would mean a route wired to a handler nobody meant to expose, which
// stays invisible until something calls it.
#[tokio::test]
async fn the_router_answers_only_the_paths_it_declares() {
    let agent_state = state().await;
    let server = Server::new(agent_state, ai(), &AgentName::new("a2a_fixture_agent"), 0)
        .expect("a registered agent should produce a server");

    let resp = server
        .create_router()
        .oneshot(
            Request::builder()
                .uri("/no-such-path")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(resp.status().as_u16(), 404);
}

// Why: the hand-written Debug impl exists because the server holds a pool, a
// JWT provider and an AI client, all of which would otherwise print their
// contents. It must stay a redaction: what it names is a fixed set of
// placeholders plus the two values that are safe and diagnostically useful,
// the port and the free stream permits.
#[tokio::test]
async fn the_debug_impl_redacts_every_handle_it_holds() {
    let agent_state = state().await;
    let server = Server::new(
        agent_state,
        ai(),
        &AgentName::new("a2a_fixture_agent"),
        4711,
    )
    .expect("a registered agent should produce a server");

    let rendered = format!("{server:?}");

    assert!(rendered.starts_with("Server {"), "got {rendered}");
    assert!(
        rendered.contains("port: 4711"),
        "the port is the field that makes the dump useful: {rendered}"
    );
    for placeholder in [
        "Arc<RwLock<AgentConfig>>",
        "Arc<AgentOAuthState>",
        "Arc<AgentState>",
        "<DynAiProvider>",
    ] {
        assert!(
            rendered.contains(placeholder),
            "{placeholder} must be redacted to a placeholder, got {rendered}"
        );
    }
    assert!(
        rendered.contains("stream_semaphore:"),
        "the permit count is reported, not the semaphore: {rendered}"
    );
    assert!(
        !rendered.contains("postgres://") && !rendered.contains("password"),
        "no connection detail may reach a debug dump: {rendered}"
    );
}

fn non_loopback_interface() -> std::net::IpAddr {
    let probe = std::net::UdpSocket::bind("0.0.0.0:0").expect("udp bind");
    probe
        .connect("192.0.2.1:9")
        .expect("route to a non-loopback address");
    let ip = probe.local_addr().expect("local addr").ip();
    assert!(
        !ip.is_loopback(),
        "the host has a non-loopback interface: {ip}"
    );
    ip
}

#[tokio::test]
async fn the_a2a_server_listens_on_loopback_only() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("free port")
        .local_addr()
        .expect("addr")
        .port();
    let server = Server::new(
        state().await,
        ai(),
        &AgentName::new("a2a_fixture_agent"),
        port,
    )
    .expect("a registered agent should produce a server");
    let task = tokio::spawn(server.run(std::future::pending::<()>()));

    let mut reachable = false;
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            reachable = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(reachable, "loopback reaches the A2A server");

    let external = non_loopback_interface();
    let refused = tokio::net::TcpStream::connect((external, port)).await;
    task.abort();
    assert!(
        refused.is_err(),
        "{external}:{port} must not reach the A2A server"
    );
}
