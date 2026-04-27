use anyhow::Result;
use async_trait::async_trait;
use futures::stream;
use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use systemprompt_identifiers::AgentName;
use systemprompt_models::ai::provider_trait::GenerateResponseParams;
use systemprompt_models::ai::tools::{CallToolResult, ToolCall};
use systemprompt_models::ai::{
    AiProvider, AiRequest, AiResponse, GoogleSearchParams, McpTool, PlanningResult,
    SearchGroundedResponse, StreamChunk, ToolModelOverrides,
};
use systemprompt_models::execution::context::RequestContext;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum MockAiCall {
    Generate { request: AiRequest },
    GenerateStream { request: AiRequest },
    GenerateWithTools { request: AiRequest },
    GenerateWithToolsStream { request: AiRequest },
    GenerateSingleTurn { request: AiRequest },
    ExecuteTools { tool_call_count: usize },
    ListAvailableTools { agent_name: String },
    GenerateWithGoogleSearch,
    HealthCheck,
    GeneratePlan { request: AiRequest },
    GenerateResponse { execution_summary: String },
}

pub struct MockAiProvider {
    generate_responses: Arc<Mutex<VecDeque<Result<AiResponse>>>>,
    generate_with_tools_responses: Arc<Mutex<VecDeque<Result<AiResponse>>>>,
    single_turn_responses: Arc<Mutex<VecDeque<Result<(AiResponse, Vec<ToolCall>)>>>>,
    health_check_responses: Arc<Mutex<VecDeque<Result<HashMap<String, bool>>>>>,
    plan_responses: Arc<Mutex<VecDeque<Result<PlanningResult>>>>,
    generate_response_responses: Arc<Mutex<VecDeque<Result<String>>>>,
    calls: Arc<Mutex<Vec<MockAiCall>>>,
    default_provider: String,
    default_model: String,
    default_max_output_tokens: u32,
}

impl std::fmt::Debug for MockAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockAiProvider")
            .field("default_provider", &self.default_provider)
            .field("default_model", &self.default_model)
            .field("default_max_output_tokens", &self.default_max_output_tokens)
            .finish()
    }
}

impl MockAiProvider {
    pub fn builder() -> MockAiProviderBuilder {
        MockAiProviderBuilder::default()
    }

    pub fn calls(&self) -> Vec<MockAiCall> {
        self.calls.lock().expect("lock poisoned").clone()
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().expect("lock poisoned").len()
    }

    pub fn reset(&self) {
        self.calls.lock().expect("lock poisoned").clear();
    }

    fn record_call(&self, call: MockAiCall) {
        self.calls.lock().expect("lock poisoned").push(call);
    }

    fn next_generate_response(&self) -> Result<AiResponse> {
        self.generate_responses
            .lock()
            .expect("lock poisoned")
            .pop_front()
            .unwrap_or_else(|| Ok(self.stub_response()))
    }

    fn next_generate_with_tools_response(&self) -> Result<AiResponse> {
        self.generate_with_tools_responses
            .lock()
            .expect("lock poisoned")
            .pop_front()
            .unwrap_or_else(|| Ok(self.stub_response()))
    }

    fn next_single_turn_response(&self) -> Result<(AiResponse, Vec<ToolCall>)> {
        self.single_turn_responses
            .lock()
            .expect("lock poisoned")
            .pop_front()
            .unwrap_or_else(|| Ok((self.stub_response(), Vec::new())))
    }

    fn next_health_check_response(&self) -> Result<HashMap<String, bool>> {
        self.health_check_responses
            .lock()
            .expect("lock poisoned")
            .pop_front()
            .unwrap_or_else(|| {
                let mut map = HashMap::new();
                map.insert(self.default_provider.clone(), true);
                Ok(map)
            })
    }

    fn next_plan_response(&self) -> Result<PlanningResult> {
        self.plan_responses
            .lock()
            .expect("lock poisoned")
            .pop_front()
            .unwrap_or_else(|| Ok(PlanningResult::direct_response("mock plan response")))
    }

    fn next_generate_response_response(&self) -> Result<String> {
        self.generate_response_responses
            .lock()
            .expect("lock poisoned")
            .pop_front()
            .unwrap_or_else(|| Ok("mock response".to_string()))
    }

    fn stub_response(&self) -> AiResponse {
        AiResponse::new(
            Uuid::new_v4(),
            "mock response".to_string(),
            self.default_provider.clone(),
            self.default_model.clone(),
        )
    }
}

impl Default for MockAiProvider {
    fn default() -> Self {
        Self {
            generate_responses: Arc::new(Mutex::new(VecDeque::new())),
            generate_with_tools_responses: Arc::new(Mutex::new(VecDeque::new())),
            single_turn_responses: Arc::new(Mutex::new(VecDeque::new())),
            health_check_responses: Arc::new(Mutex::new(VecDeque::new())),
            plan_responses: Arc::new(Mutex::new(VecDeque::new())),
            generate_response_responses: Arc::new(Mutex::new(VecDeque::new())),
            calls: Arc::new(Mutex::new(Vec::new())),
            default_provider: "mock-provider".to_string(),
            default_model: "mock-model".to_string(),
            default_max_output_tokens: 4096,
        }
    }
}

#[async_trait]
impl AiProvider for MockAiProvider {
    fn default_provider(&self) -> &str {
        &self.default_provider
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn default_max_output_tokens(&self) -> u32 {
        self.default_max_output_tokens
    }

    async fn generate(&self, request: &AiRequest) -> Result<AiResponse> {
        self.record_call(MockAiCall::Generate {
            request: request.clone(),
        });
        self.next_generate_response()
    }

    async fn generate_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<StreamChunk>> + Send>>> {
        self.record_call(MockAiCall::GenerateStream {
            request: request.clone(),
        });
        Ok(Box::pin(stream::empty()))
    }

    async fn generate_with_tools(&self, request: &AiRequest) -> Result<AiResponse> {
        self.record_call(MockAiCall::GenerateWithTools {
            request: request.clone(),
        });
        self.next_generate_with_tools_response()
    }

    async fn generate_with_tools_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<StreamChunk>> + Send>>> {
        self.record_call(MockAiCall::GenerateWithToolsStream {
            request: request.clone(),
        });
        Ok(Box::pin(stream::empty()))
    }

    async fn generate_single_turn(
        &self,
        request: &AiRequest,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        self.record_call(MockAiCall::GenerateSingleTurn {
            request: request.clone(),
        });
        self.next_single_turn_response()
    }

    async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        _tools: &[McpTool],
        _context: &RequestContext,
        _agent_overrides: Option<&ToolModelOverrides>,
    ) -> (Vec<ToolCall>, Vec<CallToolResult>) {
        let count = tool_calls.len();
        self.record_call(MockAiCall::ExecuteTools {
            tool_call_count: count,
        });
        (tool_calls, Vec::new())
    }

    async fn list_available_tools_for_agent(
        &self,
        agent_name: &AgentName,
        _context: &RequestContext,
    ) -> Result<Vec<McpTool>> {
        self.record_call(MockAiCall::ListAvailableTools {
            agent_name: agent_name.to_string(),
        });
        Ok(Vec::new())
    }

    async fn generate_with_google_search(
        &self,
        _params: GoogleSearchParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        self.record_call(MockAiCall::GenerateWithGoogleSearch);
        Ok(SearchGroundedResponse {
            content: "mock search response".to_string(),
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

    async fn health_check(&self) -> Result<HashMap<String, bool>> {
        self.record_call(MockAiCall::HealthCheck);
        self.next_health_check_response()
    }

    async fn generate_plan(
        &self,
        request: &AiRequest,
        _available_tools: &[McpTool],
    ) -> Result<PlanningResult> {
        self.record_call(MockAiCall::GeneratePlan {
            request: request.clone(),
        });
        self.next_plan_response()
    }

    async fn generate_response(&self, params: GenerateResponseParams<'_>) -> Result<String> {
        self.record_call(MockAiCall::GenerateResponse {
            execution_summary: params.execution_summary.to_string(),
        });
        self.next_generate_response_response()
    }
}

#[derive(Default)]
pub struct MockAiProviderBuilder {
    generate_responses: VecDeque<Result<AiResponse>>,
    generate_with_tools_responses: VecDeque<Result<AiResponse>>,
    single_turn_responses: VecDeque<Result<(AiResponse, Vec<ToolCall>)>>,
    health_check_responses: VecDeque<Result<HashMap<String, bool>>>,
    plan_responses: VecDeque<Result<PlanningResult>>,
    generate_response_responses: VecDeque<Result<String>>,
    default_provider: Option<String>,
    default_model: Option<String>,
    default_max_output_tokens: Option<u32>,
}

impl MockAiProviderBuilder {
    pub fn with_generate_response(mut self, response: Result<AiResponse>) -> Self {
        self.generate_responses.push_back(response);
        self
    }

    pub fn with_generate_error(mut self, error: anyhow::Error) -> Self {
        self.generate_responses.push_back(Err(error));
        self
    }

    pub fn with_generate_with_tools_response(mut self, response: Result<AiResponse>) -> Self {
        self.generate_with_tools_responses.push_back(response);
        self
    }

    pub fn with_single_turn_response(
        mut self,
        response: Result<(AiResponse, Vec<ToolCall>)>,
    ) -> Self {
        self.single_turn_responses.push_back(response);
        self
    }

    pub fn with_health_check_response(mut self, response: Result<HashMap<String, bool>>) -> Self {
        self.health_check_responses.push_back(response);
        self
    }

    pub fn with_plan_response(mut self, response: Result<PlanningResult>) -> Self {
        self.plan_responses.push_back(response);
        self
    }

    pub fn with_generate_response_response(mut self, response: Result<String>) -> Self {
        self.generate_response_responses.push_back(response);
        self
    }

    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.default_provider = Some(provider.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    pub fn with_max_output_tokens(mut self, tokens: u32) -> Self {
        self.default_max_output_tokens = Some(tokens);
        self
    }

    pub fn build(self) -> MockAiProvider {
        MockAiProvider {
            generate_responses: Arc::new(Mutex::new(self.generate_responses)),
            generate_with_tools_responses: Arc::new(Mutex::new(self.generate_with_tools_responses)),
            single_turn_responses: Arc::new(Mutex::new(self.single_turn_responses)),
            health_check_responses: Arc::new(Mutex::new(self.health_check_responses)),
            plan_responses: Arc::new(Mutex::new(self.plan_responses)),
            generate_response_responses: Arc::new(Mutex::new(self.generate_response_responses)),
            calls: Arc::new(Mutex::new(Vec::new())),
            default_provider: self
                .default_provider
                .unwrap_or_else(|| "mock-provider".to_string()),
            default_model: self
                .default_model
                .unwrap_or_else(|| "mock-model".to_string()),
            default_max_output_tokens: self.default_max_output_tokens.unwrap_or(4096),
        }
    }
}
