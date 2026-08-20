//! Persisted AI-request record and its builder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    Actor, AiRequestId, ContextId, GatewayConversationId, McpExecutionId, ProviderRequestId,
    SessionId, TaskId, TraceId, UserId,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct TokenInfo {
    pub tokens_used: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheInfo {
    pub hit: bool,
    pub read_tokens: Option<i32>,
    pub creation_tokens: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStatus {
    Pending,
    Completed,
    Failed,
    Rejected,
}

impl RequestStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiRequestRecord {
    pub request_id: AiRequestId,
    pub user_id: UserId,
    pub actor: Actor,
    pub session_id: Option<SessionId>,
    pub task_id: Option<TaskId>,
    pub context_id: ContextId,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub provider_request_id: Option<ProviderRequestId>,
    pub trace_id: Option<TraceId>,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub requested_model: Option<String>,
    pub max_tokens: Option<i32>,
    pub tokens: TokenInfo,
    pub cache: CacheInfo,
    pub is_streaming: bool,
    pub cost_microdollars: i64,
    pub latency_ms: i32,
    pub status: RequestStatus,
    pub error_message: Option<String>,
}

impl AiRequestRecord {
    pub fn builder(
        request_id: AiRequestId,
        user_id: UserId,
        context_id: ContextId,
    ) -> AiRequestRecordBuilder {
        AiRequestRecordBuilder::new(request_id, user_id, context_id)
    }
}

#[derive(Debug)]
pub struct AiRequestRecordBuilder {
    request_id: AiRequestId,
    user_id: UserId,
    actor: Option<Actor>,
    session_id: Option<SessionId>,
    task_id: Option<TaskId>,
    context_id: ContextId,
    gateway_conversation_id: Option<GatewayConversationId>,
    provider_request_id: Option<ProviderRequestId>,
    trace_id: Option<TraceId>,
    mcp_execution_id: Option<McpExecutionId>,
    provider: Option<String>,
    model: Option<String>,
    requested_model: Option<String>,
    max_tokens: Option<i32>,
    tokens: TokenInfo,
    cache: CacheInfo,
    is_streaming: bool,
    cost_microdollars: i64,
    latency_ms: i32,
    status: RequestStatus,
    error_message: Option<String>,
}

impl AiRequestRecordBuilder {
    pub fn new(request_id: AiRequestId, user_id: UserId, context_id: ContextId) -> Self {
        Self {
            request_id,
            user_id,
            actor: None,
            session_id: None,
            task_id: None,
            context_id,
            gateway_conversation_id: None,
            provider_request_id: None,
            trace_id: None,
            mcp_execution_id: None,
            provider: None,
            model: None,
            requested_model: None,
            max_tokens: None,
            tokens: TokenInfo::default(),
            cache: CacheInfo::default(),
            is_streaming: false,
            cost_microdollars: 0,
            latency_ms: 0,
            status: RequestStatus::Pending,
            error_message: None,
        }
    }

    #[must_use]
    pub fn actor(mut self, actor: Actor) -> Self {
        self.actor = Some(actor);
        self
    }

    pub fn session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn task_id(mut self, task_id: TaskId) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn gateway_conversation_id(mut self, id: GatewayConversationId) -> Self {
        self.gateway_conversation_id = Some(id);
        self
    }

    pub fn provider_request_id(mut self, id: ProviderRequestId) -> Self {
        self.provider_request_id = Some(id);
        self
    }

    pub fn trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn mcp_execution_id(mut self, mcp_execution_id: McpExecutionId) -> Self {
        self.mcp_execution_id = Some(mcp_execution_id);
        self
    }

    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    #[must_use]
    pub fn requested_model(mut self, requested_model: impl Into<String>) -> Self {
        self.requested_model = Some(requested_model.into());
        self
    }

    pub const fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens as i32);
        self
    }

    pub const fn tokens(mut self, input: Option<i32>, output: Option<i32>) -> Self {
        self.tokens.input_tokens = input;
        self.tokens.output_tokens = output;
        self.tokens.tokens_used = match (input, output) {
            (Some(i), Some(o)) => Some(i + o),
            (Some(i), None) => Some(i),
            (None, Some(o)) => Some(o),
            (None, None) => None,
        };
        self
    }

    pub const fn cache(
        mut self,
        hit: bool,
        read_tokens: Option<i32>,
        creation_tokens: Option<i32>,
    ) -> Self {
        self.cache.hit = hit;
        self.cache.read_tokens = read_tokens;
        self.cache.creation_tokens = creation_tokens;
        self
    }

    pub const fn streaming(mut self, is_streaming: bool) -> Self {
        self.is_streaming = is_streaming;
        self
    }

    pub const fn cost(mut self, cost_microdollars: i64) -> Self {
        self.cost_microdollars = cost_microdollars;
        self
    }

    pub const fn latency(mut self, latency_ms: i32) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub const fn completed(mut self) -> Self {
        self.status = RequestStatus::Completed;
        self
    }

    pub fn failed(mut self, error_message: impl Into<String>) -> Self {
        self.status = RequestStatus::Failed;
        self.error_message = Some(error_message.into());
        self
    }

    pub const fn rejected(mut self) -> Self {
        self.status = RequestStatus::Rejected;
        self
    }

    #[must_use]
    pub fn build(self) -> AiRequestRecord {
        let actor = self
            .actor
            .unwrap_or_else(|| Actor::user(self.user_id.clone()));
        AiRequestRecord {
            request_id: self.request_id,
            user_id: self.user_id,
            actor,
            session_id: self.session_id,
            task_id: self.task_id,
            context_id: self.context_id,
            gateway_conversation_id: self.gateway_conversation_id,
            provider_request_id: self.provider_request_id,
            trace_id: self.trace_id,
            mcp_execution_id: self.mcp_execution_id,
            provider: self.provider,
            model: self.model,
            requested_model: self.requested_model,
            max_tokens: self.max_tokens,
            tokens: self.tokens,
            cache: self.cache,
            is_streaming: self.is_streaming,
            cost_microdollars: self.cost_microdollars,
            latency_ms: self.latency_ms,
            status: self.status,
            error_message: self.error_message,
        }
    }
}
