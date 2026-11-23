//! [`InferenceNotConfigured`]: the AI provider the `ai` member provides when
//! the booted catalog serves no default model.
//!
//! The member always provides one `DynAiProvider`; when the service cannot be
//! built (no servable default model, a bridge-only deployment, or no AI
//! session provider admitted) every call answers
//! [`AiInferenceError::Configuration`] carrying the reason as its source, and
//! tool execution returns no result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;
use systemprompt_identifiers::AgentName;
use systemprompt_models::ai::{
    AiProvider, AiRequest, AiResponse, CallToolResult, GenerateResponseParams,
    GoogleSearchParams, McpTool, PlanningResult, SearchGroundedResponse, StreamChunk, ToolCall,
    ToolModelOverrides,
};
use systemprompt_models::errors::{AiInferenceError, AiInferenceResult};
use systemprompt_models::execution::context::RequestContext;

use crate::error::AiError;

/// Inference is not available in this deployment.
#[derive(Debug, Clone)]
pub struct InferenceNotConfigured {
    cause: InferenceUnavailable,
}

/// Why an [`InferenceNotConfigured`] provider refuses.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InferenceUnavailable {
    #[error("inference is not configured in this deployment")]
    NotConfigured(#[source] Arc<AiError>),
    #[error("inference needs the AI session provider the `users` member provides")]
    NoSessionProvider,
}

impl InferenceNotConfigured {
    #[must_use]
    pub const fn new(cause: InferenceUnavailable) -> Self {
        Self { cause }
    }

    fn refused(&self) -> AiInferenceError {
        AiInferenceError::Configuration(Box::new(self.cause.clone()))
    }
}

type ChunkStream = Pin<Box<dyn Stream<Item = AiInferenceResult<StreamChunk>> + Send>>;

#[async_trait]
impl AiProvider for InferenceNotConfigured {
    fn default_provider(&self) -> &'static str {
        ""
    }

    fn default_model(&self) -> &'static str {
        ""
    }

    fn default_max_output_tokens(&self) -> u32 {
        0
    }

    async fn generate(&self, _: &AiRequest) -> AiInferenceResult<AiResponse> {
        Err(self.refused())
    }

    async fn generate_stream(&self, _: &AiRequest) -> AiInferenceResult<ChunkStream> {
        Err(self.refused())
    }

    async fn generate_with_tools(&self, _: &AiRequest) -> AiInferenceResult<AiResponse> {
        Err(self.refused())
    }

    async fn generate_with_tools_stream(&self, _: &AiRequest) -> AiInferenceResult<ChunkStream> {
        Err(self.refused())
    }

    async fn generate_single_turn(
        &self,
        _: &AiRequest,
    ) -> AiInferenceResult<(AiResponse, Vec<ToolCall>)> {
        Err(self.refused())
    }

    async fn execute_tools(
        &self,
        _: Vec<ToolCall>,
        _: &[McpTool],
        _: &RequestContext,
        _: Option<&ToolModelOverrides>,
    ) -> (Vec<ToolCall>, Vec<CallToolResult>) {
        (Vec::new(), Vec::new())
    }

    async fn list_available_tools_for_agent(
        &self,
        _: &AgentName,
        _: &RequestContext,
    ) -> AiInferenceResult<Vec<McpTool>> {
        Err(self.refused())
    }

    async fn generate_with_google_search(
        &self,
        _: GoogleSearchParams<'_>,
    ) -> AiInferenceResult<SearchGroundedResponse> {
        Err(self.refused())
    }

    async fn health_check(&self) -> AiInferenceResult<HashMap<String, bool>> {
        Err(self.refused())
    }

    async fn generate_plan(
        &self,
        _: &AiRequest,
        _: &[McpTool],
    ) -> AiInferenceResult<PlanningResult> {
        Err(self.refused())
    }

    async fn generate_response(&self, _: GenerateResponseParams<'_>) -> AiInferenceResult<String> {
        Err(self.refused())
    }
}
