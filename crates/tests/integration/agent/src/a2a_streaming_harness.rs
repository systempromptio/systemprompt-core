//! In-process A2A streaming harness — assembles a minimal
//! [`AgentHandlerState`] over a fixture-backed `DbPool` and drives
//! [`create_sse_stream`]. The primary objective is the
//! semaphore-rejection path, which is deterministic and proves the
//! harness wiring without depending on agent-registry filesystem state.
//!
//! Deeper success paths (planning → tool execution → completion) require
//! a real `AgentRegistry`-backed `AgentConfig` plus an MCP service
//! provider; those are scaffolded but currently exercise the failure
//! branch — that still covers `setup_stream` initialization code that
//! was previously unreached.

use std::sync::Arc;

use systemprompt_agent::AgentState;
use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::auth::{AgentOAuthConfig, AgentOAuthState};
use systemprompt_agent::services::a2a_server::handlers::AgentHandlerState;
use systemprompt_agent::services::a2a_server::streaming::{
    CreateSseStreamParams, StreamRejected, create_sse_stream,
};
use systemprompt_identifiers::{
    AgentName, ContextId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentOAuthConfig as AgentConfigOAuth,
    AiProvider, CapabilitiesConfig,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};
use systemprompt_test_mocks::MockAiProvider;
use systemprompt_traits::{
    AgentJwtClaims, DynJwtValidationProvider, GenerateTokenParams, JwtProviderError, JwtResult,
    JwtValidationProvider,
};
use tokio::sync::{RwLock, Semaphore};

struct StubJwtProvider;

impl JwtValidationProvider for StubJwtProvider {
    fn validate_token(&self, _token: &str) -> JwtResult<AgentJwtClaims> {
        Err(JwtProviderError::InvalidToken)
    }

    fn generate_token(&self, _params: GenerateTokenParams) -> JwtResult<String> {
        Ok("stub-token".to_owned())
    }

    fn generate_secure_token(&self, prefix: &str) -> String {
        format!("{prefix}-stub")
    }
}

fn fixture_agent_config() -> AgentConfig {
    AgentConfig {
        name: "test_agent".to_owned(),
        port: 4000,
        endpoint: "http://localhost:4000/test_agent".to_owned(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: Vec::new(),
        card: AgentCardConfig {
            protocol_version: "0.2.3".to_owned(),
            name: Some("test_agent".to_owned()),
            display_name: "Test Agent".to_owned(),
            description: "harness".to_owned(),
            version: "1.0.0".to_owned(),
            preferred_transport: "JSONRPC".to_owned(),
            icon_url: None,
            documentation_url: None,
            provider: None,
            capabilities: CapabilitiesConfig::default(),
            default_input_modes: vec!["text/plain".to_owned()],
            default_output_modes: vec!["text/plain".to_owned()],
            security_schemes: None,
            security: None,
            skills: vec![],
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: AgentConfigOAuth::default(),
    }
}

async fn build_state(permits: usize) -> anyhow::Result<Arc<AgentHandlerState>> {
    let bootstrap = ensure_test_bootstrap();
    let db_pool = fixture_db_pool(&bootstrap.database_url).await?;

    let global_config = Arc::new(systemprompt_test_fixtures::fixture_config(
        &bootstrap.database_url,
    ));

    let jwt_provider: DynJwtValidationProvider = Arc::new(StubJwtProvider);

    let agent_state = Arc::new(AgentState::new(
        Arc::clone(&db_pool),
        Arc::clone(&global_config),
        Arc::clone(&jwt_provider),
    ));

    let oauth_state = Arc::new(
        AgentOAuthState::new(
            Arc::clone(&db_pool),
            AgentOAuthConfig::default(),
            global_config.jwt_issuer.clone(),
            global_config.jwt_audiences.clone(),
        )
        .with_jwt_provider(Arc::clone(&jwt_provider)),
    );

    let ai_service: Arc<dyn AiProvider> = Arc::new(MockAiProvider::default());

    Ok(Arc::new(AgentHandlerState {
        db_pool,
        config: Arc::new(RwLock::new(fixture_agent_config())),
        oauth_state,
        agent_state,
        ai_service,
        stream_semaphore: Arc::new(Semaphore::new(permits)),
    }))
}

fn user_message(text: &str) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: text.to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn fixture_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::new("trace-harness"),
        ContextId::generate(),
        AgentName::new("test_agent"),
    )
}

#[tokio::test]
async fn create_sse_stream_with_exhausted_semaphore_returns_rejected() -> anyhow::Result<()> {
    let state = build_state(0).await?;
    let result = create_sse_stream(CreateSseStreamParams {
        message: user_message("hello"),
        agent_name: "test_agent".to_owned(),
        state: Arc::clone(&state),
        request_id: NumberOrString::Number(1),
        context: fixture_request_context(),
        callback_config: None,
    })
    .await;
    assert!(
        matches!(result, Err(StreamRejected)),
        "expected StreamRejected when semaphore has 0 permits"
    );
    Ok(())
}

#[tokio::test]
async fn create_sse_stream_with_available_permit_returns_receiver_stream() -> anyhow::Result<()> {
    let state = build_state(1).await?;
    let result = create_sse_stream(CreateSseStreamParams {
        message: user_message("hello"),
        agent_name: "test_agent".to_owned(),
        state: Arc::clone(&state),
        request_id: NumberOrString::Number(2),
        context: fixture_request_context(),
        callback_config: None,
    })
    .await;
    // The spawned setup_stream task will fail (no MCP service provider, no
    // task repo bootstrap data), but `create_sse_stream` itself returns Ok
    // immediately after acquiring the permit and spawning. This covers the
    // happy-path entry through the semaphore and channel creation.
    assert!(
        result.is_ok(),
        "create_sse_stream returned Err with available permit"
    );
    // Drop the receiver so the spawned task's error-handling path runs to
    // completion (drains the channel send attempts).
    drop(result);
    Ok(())
}

#[tokio::test]
async fn semaphore_releases_permit_after_receiver_dropped() -> anyhow::Result<()> {
    let state = build_state(1).await?;
    let initial_permits = state.stream_semaphore.available_permits();
    assert_eq!(initial_permits, 1);

    let stream = create_sse_stream(CreateSseStreamParams {
        message: user_message("first"),
        agent_name: "test_agent".to_owned(),
        state: Arc::clone(&state),
        request_id: NumberOrString::Number(10),
        context: fixture_request_context(),
        callback_config: None,
    })
    .await;
    assert!(stream.is_ok());
    // While the spawned task is alive, the permit is held.
    drop(stream);
    // Give the spawned task a moment to drop its permit.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    // The permit should have been released — the next call should succeed
    // without rejection.
    let second = create_sse_stream(CreateSseStreamParams {
        message: user_message("second"),
        agent_name: "test_agent".to_owned(),
        state: Arc::clone(&state),
        request_id: NumberOrString::Number(11),
        context: fixture_request_context(),
        callback_config: None,
    })
    .await;
    assert!(
        second.is_ok(),
        "expected permit to be released, got {:?}",
        second.err()
    );
    Ok(())
}
