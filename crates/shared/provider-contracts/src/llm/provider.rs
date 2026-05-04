//! [`LlmProvider`] and tool-execution traits + their associated context types.

use async_trait::async_trait;
use futures::stream::Stream;
use serde_json::Value as JsonValue;
use std::pin::Pin;
use systemprompt_identifiers::{SessionId, TraceId};

use super::error::LlmProviderResult;
use super::request::ChatRequest;
use super::response::ChatResponse;
use crate::tool::{ToolCallRequest, ToolCallResult, ToolDefinition};

/// Streaming chat-completion item stream.
///
/// Each yielded `String` is a partial chunk of the assistant's reply.
/// Internally the items are typed with `anyhow::Result` so providers can
/// surface arbitrary upstream stream errors without contorting their
/// signatures; the chunk consumer typically converts these into
/// [`LlmProviderError::Internal`](super::error::LlmProviderError::Internal).
pub type ChatStream = Pin<Box<dyn Stream<Item = anyhow::Result<String>> + Send>>;

/// Provider-agnostic chat-completion contract.
///
/// Marked `#[async_trait]` because it is consumed via `dyn LlmProvider`
/// across crate boundaries.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Issue a single non-streaming chat call.
    async fn chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatResponse>;

    /// Issue a streaming chat call, returning a stream of token chunks.
    async fn stream_chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatStream>;

    /// Provider's preferred default model when none is specified.
    fn default_model(&self) -> &str;

    /// Whether the provider supports the given model identifier.
    fn supports_model(&self, model: &str) -> bool;

    /// Whether [`LlmProvider::stream_chat`] is meaningfully implemented.
    fn supports_streaming(&self) -> bool;

    /// Whether the provider can route tool calls back to the host.
    fn supports_tools(&self) -> bool;
}

/// Executes the tool calls a model emits during a chat turn.
///
/// Marked `#[async_trait]` because it is consumed via `dyn ToolExecutor`.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute the given tool calls and return the matching results.
    ///
    /// Returns the (possibly filtered / re-ordered) tool calls plus the
    /// corresponding results in the same order so callers can pair them up.
    async fn execute(
        &self,
        tool_calls: Vec<ToolCallRequest>,
        tools: &[ToolDefinition],
        context: &ToolExecutionContext,
    ) -> (Vec<ToolCallRequest>, Vec<ToolCallResult>);
}

/// Per-call context handed to [`ToolExecutor::execute`].
#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    /// Bearer token forwarded to the tool service.
    pub auth_token: String,
    /// Originating session, when known.
    pub session_id: Option<SessionId>,
    /// Originating trace, when known.
    pub trace_id: Option<TraceId>,
    /// Provider-specific model overrides serialized as JSON.
    pub model_overrides: Option<JsonValue>,
}

impl ToolExecutionContext {
    /// Build a [`ToolExecutionContext`] with only the auth token populated.
    #[must_use]
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            model_overrides: None,
        }
    }

    /// Attach a [`SessionId`].
    #[must_use]
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Attach a [`TraceId`].
    #[must_use]
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    /// Attach a JSON blob of model-config overrides.
    #[must_use]
    pub fn with_model_overrides(mut self, overrides: JsonValue) -> Self {
        self.model_overrides = Some(overrides);
        self
    }
}
