//! LLM provider traits for abstracting AI model interactions.
//!
//! This module defines traits for LLM (Large Language Model) providers,
//! allowing other modules to depend on abstract AI capabilities rather
//! than concrete implementations.

use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::pin::Pin;

use crate::tool_provider::{ToolCallRequest, ToolCallResult, ToolDefinition};

/// A message in a conversation with an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// The role of the message sender (user, assistant, system)
    pub role: ChatRole,
    /// The content of the message
    pub content: String,
}

impl ChatMessage {
    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }
}

/// Role of a message sender in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    /// System instructions
    System,
    /// User input
    User,
    /// Assistant response
    Assistant,
    /// Tool result
    Tool,
}

/// Parameters for controlling LLM sampling behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SamplingParameters {
    /// Temperature for randomness (0.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p (nucleus) sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k sampling
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

/// A request to an LLM provider.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// The conversation messages
    pub messages: Vec<ChatMessage>,
    /// The model to use
    pub model: String,
    /// Maximum tokens to generate
    pub max_output_tokens: u32,
    /// Sampling parameters
    pub sampling: Option<SamplingParameters>,
    /// Available tools (if any)
    pub tools: Option<Vec<ToolDefinition>>,
    /// JSON schema for structured output (if any)
    pub response_schema: Option<JsonValue>,
}

impl ChatRequest {
    /// Create a new chat request.
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

    /// Set max output tokens.
    pub const fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_output_tokens = max_tokens;
        self
    }

    /// Set sampling parameters.
    pub const fn with_sampling(mut self, sampling: SamplingParameters) -> Self {
        self.sampling = Some(sampling);
        self
    }

    /// Set available tools.
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set response schema for structured output.
    pub fn with_response_schema(mut self, schema: JsonValue) -> Self {
        self.response_schema = Some(schema);
        self
    }
}

/// Response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// The generated text content
    pub content: String,
    /// Tool calls requested by the model (if any)
    pub tool_calls: Vec<ToolCallRequest>,
    /// Token usage statistics
    pub usage: Option<TokenUsage>,
    /// The model that was used
    pub model: String,
    /// Response latency in milliseconds
    pub latency_ms: u64,
}

impl ChatResponse {
    /// Create a new chat response.
    pub fn new(content: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: vec![],
            usage: None,
            model: model.into(),
            latency_ms: 0,
        }
    }

    /// Add tool calls to the response.
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCallRequest>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    /// Add usage statistics.
    pub const fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Set latency.
    pub const fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }
}

/// Token usage statistics.
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

/// A streaming chat response chunk.
pub type ChatStream = Pin<Box<dyn Stream<Item = anyhow::Result<String>> + Send>>;

/// Error type for LLM provider operations.
#[derive(Debug, thiserror::Error)]
pub enum LlmProviderError {
    /// Model not supported
    #[error("Model '{0}' not supported")]
    ModelNotSupported(String),

    /// Provider not available
    #[error("Provider '{0}' not available")]
    ProviderNotAvailable(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Request validation failed
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Generation failed
    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for LlmProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Result type for LLM provider operations.
pub type LlmProviderResult<T> = Result<T, LlmProviderError>;

/// Trait for LLM providers.
///
/// This trait abstracts the LLM interaction, allowing modules to use
/// AI capabilities without depending on specific implementations.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a response to a chat request.
    ///
    /// # Arguments
    /// * `request` - The chat request
    ///
    /// # Returns
    /// The chat response including any tool calls
    async fn chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatResponse>;

    /// Generate a streaming response to a chat request.
    ///
    /// # Arguments
    /// * `request` - The chat request
    ///
    /// # Returns
    /// A stream of response chunks
    async fn stream_chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatStream>;

    /// Get the default model for this provider.
    fn default_model(&self) -> &str;

    /// Check if a model is supported.
    fn supports_model(&self, model: &str) -> bool;

    /// Check if streaming is supported.
    fn supports_streaming(&self) -> bool;

    /// Check if tool use is supported.
    fn supports_tools(&self) -> bool;
}

/// Trait for executing tools during an AI conversation.
///
/// This trait is implemented by services that can execute tools
/// and integrate the results back into the conversation.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a list of tool calls.
    ///
    /// # Arguments
    /// * `tool_calls` - The tool calls to execute
    /// * `tools` - The available tool definitions
    /// * `context` - Additional context for execution
    ///
    /// # Returns
    /// The executed tool calls paired with their results
    async fn execute(
        &self,
        tool_calls: Vec<ToolCallRequest>,
        tools: &[ToolDefinition],
        context: &ToolExecutionContext,
    ) -> (Vec<ToolCallRequest>, Vec<ToolCallResult>);
}

/// Context for tool execution.
#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    /// Authentication token
    pub auth_token: String,
    /// Session ID
    pub session_id: Option<String>,
    /// Trace ID
    pub trace_id: Option<String>,
    /// Model configuration overrides per tool
    pub model_overrides: Option<JsonValue>,
}

impl ToolExecutionContext {
    /// Create a new tool execution context.
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            model_overrides: None,
        }
    }
}
