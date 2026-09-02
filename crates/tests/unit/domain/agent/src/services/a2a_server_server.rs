//! `a2a_server::Server::new` — what it refuses before building anything.
//!
//! This file was deliberately kept out of the coverage ignore list on the
//! grounds that it has real seams, and then sat at 19.6%. `run` binds a
//! listener and stays out of reach; `new` and `create_router` do not. The
//! suite boots a services tree carrying an `agents:` section so the registry
//! lookup succeeds and the router can actually be built and driven.

use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::Server;
use systemprompt_database::DbPool;
use systemprompt_models::ai::AiProvider;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_config, fixture_db_pool, init_ai_bootstrap,
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
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: http://127.0.0.1:1
    api_key_secret: anthropic_api_key
    models:
      - id: claude-fixture-1
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
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
    BOOT.get_or_init(|| init_ai_bootstrap(GATEWAY_YAML, SERVICES_YAML))
}

async fn state() -> (Arc<AgentState>, DbPool) {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url)
        .await
        .expect("the a2a server tests need a reachable test database");
    let config = Arc::new(fixture_config(&b.database_url));
    let repos =
        systemprompt_agent::repository::A2ARepositories::new(&pool, crate::session_usage(&pool), systemprompt_identifiers::InstanceId::new("test-instance"))
            .expect("repositories");
    let state = AgentState::new(
        pool.clone(),
        config,
        Arc::new(StubJwt) as systemprompt_traits::DynJwtValidationProvider,
        Arc::new(repos),
    );
    (Arc::new(state), pool)
}

fn ai() -> Arc<dyn AiProvider> {
    Arc::new(MockAiProvider::builder().build())
}

// Why: the agent name selects the config the server serves — its card, its
// scopes, its identity. Starting without one would leave a server answering as
// nothing in particular, so it is refused before any state is built.
#[tokio::test]
async fn a_server_without_an_agent_name_is_refused() {
    let (agent_state, pool) = state().await;

    let err = Server::new(pool, agent_state, ai(), None, 0)
        .await
        .expect_err("a server with no agent name must not start");

    assert!(
        format!("{err}").to_lowercase().contains("agent name"),
        "the refusal should name what was missing: {err}"
    );
}

// Why: an unregistered name must fail rather than fall back to a default. A
// server that quietly served some other agent's card would answer for an
// identity nobody asked it to hold.
#[tokio::test]
async fn a_server_for_an_agent_that_is_not_registered_is_refused() {
    let (agent_state, pool) = state().await;

    let err = Server::new(
        pool,
        agent_state,
        ai(),
        Some("agent-that-does-not-exist".to_owned()),
        0,
    )
    .await
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
    let (agent_state, pool) = state().await;
    let server = Server::new(
        pool,
        agent_state,
        ai(),
        Some("a2a_fixture_agent".to_owned()),
        0,
    )
    .await
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
