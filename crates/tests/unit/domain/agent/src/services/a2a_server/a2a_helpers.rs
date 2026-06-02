// Shared construction helpers for the A2A-server runtime tests: a pooled
// `AgentHandlerState`, a stubbed `AiProvider`, and request/runtime builders.
//
// Every entry point early-returns at the call site when no test database is
// configured (via `try_pool`); these helpers assume a live pool was obtained.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_agent::AgentState;
use systemprompt_agent::models::AgentRuntimeInfo;
use systemprompt_agent::services::a2a_server::auth::{AgentOAuthConfig, AgentOAuthState};
use systemprompt_agent::services::a2a_server::handlers::AgentHandlerState;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::ai::provider_trait::GenerateResponseParams;
use systemprompt_models::ai::tools::{CallToolResult, ToolCall};
use systemprompt_models::ai::{
    AiProvider, AiRequest, AiResponse, GoogleSearchParams, McpTool, PlanningResult,
    SearchGroundedResponse, StreamChunk,
};
use systemprompt_models::ai::ToolModelOverrides;
use systemprompt_models::errors::ProviderResult;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::services::PluginComponentRef;
use systemprompt_models::AiMessage;
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

// A JWT provider that rejects every token; sufficient for state construction.
struct RejectingJwtProvider;

impl JwtValidationProvider for RejectingJwtProvider {
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

pub(crate) fn stub_jwt() -> systemprompt_traits::DynJwtValidationProvider {
    Arc::new(RejectingJwtProvider)
}

// A configurable in-test Ai provider. Streams are queued as chunk batches;
// `generate` returns a queued response or a canned default.
pub(crate) struct StubAiProvider {
    generate_responses: Mutex<Vec<ProviderResult<AiResponse>>>,
    stream_chunks: Mutex<Vec<Vec<ProviderResult<StreamChunk>>>>,
    fail_stream: bool,
    provider: String,
    model: String,
    max_tokens: u32,
}

impl std::fmt::Debug for StubAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StubAiProvider").finish()
    }
}

impl StubAiProvider {
    pub(crate) fn new() -> Self {
        Self {
            generate_responses: Mutex::new(Vec::new()),
            stream_chunks: Mutex::new(Vec::new()),
            fail_stream: false,
            provider: "mock-provider".to_owned(),
            model: "mock-model".to_owned(),
            max_tokens: 4096,
        }
    }

    pub(crate) fn with_generate(mut self, content: &str) -> Self {
        self.generate_responses
            .get_mut()
            .expect("lock")
            .push(Ok(AiResponse::new(
                Uuid::new_v4(),
                content.to_owned(),
                self.provider.clone(),
                self.model.clone(),
            )));
        self
    }

    pub(crate) fn with_text_stream(mut self, parts: &[&str]) -> Self {
        let chunks = parts
            .iter()
            .map(|p| Ok(StreamChunk::Text((*p).to_owned())))
            .collect();
        self.stream_chunks.get_mut().expect("lock").push(chunks);
        self
    }

    pub(crate) fn failing_stream(mut self) -> Self {
        self.fail_stream = true;
        self
    }

    fn next_generate(&self) -> ProviderResult<AiResponse> {
        self.generate_responses
            .lock()
            .expect("lock")
            .pop()
            .unwrap_or_else(|| {
                Ok(AiResponse::new(
                    Uuid::new_v4(),
                    "default".to_owned(),
                    self.provider.clone(),
                    self.model.clone(),
                ))
            })
    }
}

#[async_trait]
impl AiProvider for StubAiProvider {
    fn default_provider(&self) -> &str {
        &self.provider
    }
    fn default_model(&self) -> &str {
        &self.model
    }
    fn default_max_output_tokens(&self) -> u32 {
        self.max_tokens
    }

    async fn generate(&self, _request: &AiRequest) -> ProviderResult<AiResponse> {
        self.next_generate()
    }

    async fn generate_stream(
        &self,
        _request: &AiRequest,
    ) -> ProviderResult<Pin<Box<dyn futures::Stream<Item = ProviderResult<StreamChunk>> + Send>>>
    {
        if self.fail_stream {
            return Err("stub stream failure".into());
        }
        let batch = self
            .stream_chunks
            .lock()
            .expect("lock")
            .pop()
            .unwrap_or_default();
        Ok(Box::pin(futures::stream::iter(batch)))
    }

    async fn generate_with_tools(&self, _request: &AiRequest) -> ProviderResult<AiResponse> {
        self.next_generate()
    }

    async fn generate_with_tools_stream(
        &self,
        _request: &AiRequest,
    ) -> ProviderResult<Pin<Box<dyn futures::Stream<Item = ProviderResult<StreamChunk>> + Send>>>
    {
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn generate_single_turn(
        &self,
        _request: &AiRequest,
    ) -> ProviderResult<(AiResponse, Vec<ToolCall>)> {
        Ok((self.next_generate()?, Vec::new()))
    }

    async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        _tools: &[McpTool],
        _context: &RequestContext,
        _agent_overrides: Option<&ToolModelOverrides>,
    ) -> (Vec<ToolCall>, Vec<CallToolResult>) {
        (tool_calls, Vec::new())
    }

    async fn list_available_tools_for_agent(
        &self,
        _agent_name: &AgentName,
        _context: &RequestContext,
    ) -> ProviderResult<Vec<McpTool>> {
        Ok(Vec::new())
    }

    async fn generate_with_google_search(
        &self,
        _params: GoogleSearchParams<'_>,
    ) -> ProviderResult<SearchGroundedResponse> {
        Ok(SearchGroundedResponse {
            content: "search".to_owned(),
            sources: Vec::new(),
            confidence_scores: Vec::new(),
            web_search_queries: Vec::new(),
            url_context_metadata: None,
            tokens_used: None,
            latency_ms: 0,
            finish_reason: None,
            safety_ratings: None,
        })
    }

    async fn health_check(&self) -> ProviderResult<HashMap<String, bool>> {
        let mut m = HashMap::new();
        m.insert(self.provider.clone(), true);
        Ok(m)
    }

    async fn generate_plan(
        &self,
        _request: &AiRequest,
        _available_tools: &[McpTool],
    ) -> ProviderResult<PlanningResult> {
        Ok(PlanningResult::direct_response("stub plan"))
    }

    async fn generate_response(
        &self,
        _params: GenerateResponseParams<'_>,
    ) -> ProviderResult<String> {
        Ok("stub response".to_owned())
    }
}

pub(crate) fn make_agent_state(pool: &DbPool) -> Arc<AgentState> {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let url = systemprompt_test_fixtures::fixture_database_url().expect("url");
    let config = Arc::new(systemprompt_test_fixtures::fixture_config(&url));
    Arc::new(AgentState::new(Arc::clone(pool), config, stub_jwt()))
}

pub(crate) fn make_oauth_state(pool: &DbPool) -> Arc<AgentOAuthState> {
    Arc::new(AgentOAuthState::new(
        Arc::clone(pool),
        AgentOAuthConfig::default(),
        "test-issuer".to_owned(),
        vec![],
    ))
}

// Build a handler state with the given AI provider and stream permits.
pub(crate) fn make_handler_state(
    pool: &DbPool,
    ai_service: Arc<dyn AiProvider>,
    stream_permits: usize,
) -> Arc<AgentHandlerState> {
    let agent_state = make_agent_state(pool);
    let oauth_state = make_oauth_state(pool);
    let config = Arc::new(RwLock::new(agent_config("test_agent")));

    Arc::new(AgentHandlerState {
        db_pool: Arc::clone(pool),
        config,
        oauth_state,
        agent_state,
        ai_service,
        stream_semaphore: Arc::new(Semaphore::new(stream_permits)),
    })
}

pub(crate) fn agent_config(name: &str) -> systemprompt_models::AgentConfig {
    use systemprompt_models::{
        AgentCardConfig, AgentMetadataConfig, CapabilitiesConfig,
    };
    systemprompt_models::AgentConfig {
        name: name.to_owned(),
        port: 9100,
        endpoint: String::new(),
        tags: Vec::new(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        card: AgentCardConfig {
            protocol_version: "1.0".to_owned(),
            name: None,
            display_name: "Test".to_owned(),
            description: "test agent".to_owned(),
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
            skills: Vec::new(),
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: systemprompt_models::AgentOAuthConfig::default(),
    }
}

pub(crate) fn runtime_info(name: &str) -> AgentRuntimeInfo {
    AgentRuntimeInfo {
        name: name.to_owned(),
        port: 0,
        is_enabled: true,
        is_primary: false,
        system_prompt: Some("You are a test agent.".to_owned()),
        mcp_servers: PluginComponentRef::default(),
        provider: Some("mock-provider".to_owned()),
        model: Some("mock-model".to_owned()),
        max_output_tokens: Some(1024),
        skills: PluginComponentRef::default(),
        tool_model_overrides: ToolModelOverrides::default(),
    }
}

pub(crate) fn request_context(
    ctx: &ContextId,
    session: &SessionId,
    user: &UserId,
    agent_name: &str,
) -> RequestContext {
    let mut rc = RequestContext::new(
        session.clone(),
        TraceId::generate(),
        ctx.clone(),
        AgentName::new(agent_name),
    );
    rc.auth.actor = systemprompt_identifiers::Actor::user(user.clone());
    rc.with_auth_token("test-token")
}

pub(crate) fn ai_messages(text: &str) -> Vec<AiMessage> {
    vec![AiMessage {
        role: systemprompt_models::MessageRole::User,
        content: text.to_owned(),
        parts: Vec::new(),
    }]
}
