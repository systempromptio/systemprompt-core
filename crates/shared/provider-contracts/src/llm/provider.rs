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

pub type ChatStream = Pin<Box<dyn Stream<Item = anyhow::Result<String>> + Send>>;

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
