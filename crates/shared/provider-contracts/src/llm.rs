use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::pin::Pin;
use systemprompt_identifiers::{SessionId, TraceId};

use crate::tool::{ToolCallRequest, ToolCallResult, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }

    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SamplingParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

impl SamplingParameters {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            temperature: None,
            top_p: None,
            top_k: None,
        }
    }

    #[must_use]
    pub const fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    #[must_use]
    pub const fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    #[must_use]
    pub const fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }
}

impl Default for SamplingParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub max_output_tokens: u32,
    pub sampling: Option<SamplingParameters>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub response_schema: Option<JsonValue>,
}

impl ChatRequest {
    #[must_use]
    pub fn new(
        messages: Vec<ChatMessage>,
        model: impl Into<String>,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            messages,
            model: model.into(),
            max_output_tokens,
            sampling: None,
            tools: None,
            response_schema: None,
        }
    }

    #[must_use]
    pub const fn with_sampling(mut self, sampling: SamplingParameters) -> Self {
        self.sampling = Some(sampling);
        self
    }

    #[must_use]
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    #[must_use]
    pub fn with_response_schema(mut self, schema: JsonValue) -> Self {
        self.response_schema = Some(schema);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCallRequest>,
    pub usage: Option<TokenUsage>,
    pub model: String,
    pub latency_ms: u64,
}

impl ChatResponse {
    #[must_use]
    pub fn new(content: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: vec![],
            usage: None,
            model: model.into(),
            latency_ms: 0,
        }
    }

    #[must_use]
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCallRequest>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    #[must_use]
    pub const fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    #[must_use]
    pub const fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(rename = "input_tokens")]
    pub input: u32,
    #[serde(rename = "output_tokens")]
    pub output: u32,
    #[serde(rename = "total_tokens")]
    pub total: u32,
    #[serde(rename = "cache_read_tokens")]
    pub cache_read: Option<u32>,
    #[serde(rename = "cache_creation_tokens")]
    pub cache_creation: Option<u32>,
}

impl TokenUsage {
    #[must_use]
    pub const fn new(input: u32, output: u32) -> Self {
        Self {
            input,
            output,
            total: input + output,
            cache_read: None,
            cache_creation: None,
        }
    }

    #[must_use]
    pub const fn with_cache_read(mut self, cache_read: u32) -> Self {
        self.cache_read = Some(cache_read);
        self
    }

    #[must_use]
    pub const fn with_cache_creation(mut self, cache_creation: u32) -> Self {
        self.cache_creation = Some(cache_creation);
        self
    }
}

pub type ChatStream = Pin<Box<dyn Stream<Item = anyhow::Result<String>> + Send>>;

#[derive(Debug, thiserror::Error)]
pub enum LlmProviderError {
    #[error("Model '{0}' not supported")]
    ModelNotSupported(String),

    #[error("Provider '{0}' not available")]
    ProviderNotAvailable(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    #[error("Internal error: {0}")]
    Internal(#[source] anyhow::Error),
}

impl From<anyhow::Error> for LlmProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

pub type LlmProviderResult<T> = Result<T, LlmProviderError>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatResponse>;

    async fn stream_chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatStream>;

    fn default_model(&self) -> &str;

    fn supports_model(&self, model: &str) -> bool;

    fn supports_streaming(&self) -> bool;

    fn supports_tools(&self) -> bool;
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(
        &self,
        tool_calls: Vec<ToolCallRequest>,
        tools: &[ToolDefinition],
        context: &ToolExecutionContext,
    ) -> (Vec<ToolCallRequest>, Vec<ToolCallResult>);
}

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub auth_token: String,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub model_overrides: Option<JsonValue>,
}

impl ToolExecutionContext {
    #[must_use]
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            model_overrides: None,
        }
    }

    #[must_use]
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    #[must_use]
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    #[must_use]
    pub fn with_model_overrides(mut self, overrides: JsonValue) -> Self {
        self.model_overrides = Some(overrides);
        self
    }
}
