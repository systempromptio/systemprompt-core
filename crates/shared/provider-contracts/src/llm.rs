use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::pin::Pin;

use crate::tool::{ToolCallRequest, ToolCallResult, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }

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

impl Default for SamplingParameters {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            top_p: None,
            top_k: None,
        }
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
    pub fn new(messages: Vec<ChatMessage>, model: impl Into<String>) -> Self {
        Self {
            messages,
            model: model.into(),
            max_output_tokens: 4096,
            sampling: None,
            tools: None,
            response_schema: None,
        }
    }

    pub const fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_output_tokens = max_tokens;
        self
    }

    pub const fn with_sampling(mut self, sampling: SamplingParameters) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

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
    pub fn new(content: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: vec![],
            usage: None,
            model: model.into(),
            latency_ms: 0,
        }
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCallRequest>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    pub const fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

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
    Internal(String),
}

impl From<anyhow::Error> for LlmProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
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
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub model_overrides: Option<JsonValue>,
}

impl ToolExecutionContext {
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            model_overrides: None,
        }
    }
}
