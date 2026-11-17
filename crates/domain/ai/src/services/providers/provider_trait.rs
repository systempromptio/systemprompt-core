//! The [`ProviderClient`] abstraction.
//!
//! Every LLM backend implements [`ProviderClient`], a `dyn`-dispatched trait
//! covering plain generation, tool calling, structured/schema output, search
//! grounding, and streaming. The borrowed parameter structs
//! ([`GenerationParams`], [`ToolGenerationParams`], [`SchemaGenerationParams`],
//! [`StructuredGenerationParams`], [`SearchGenerationParams`],
//! [`ToolResultsParams`]) live in `provider_params` and are re-exported here.
//! [`systemprompt_manifest::services::ai::ModelPricing`] is re-exported here as
//! the single pricing type for usage accounting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{AiError, ProviderCapability, Result};
use crate::models::ai::{
    AiMessage, AiResponse, EmbeddingRequest, EmbeddingResponse, SamplingParams,
    SearchGroundedResponse,
};
use crate::models::tools::ToolCall;
use crate::services::schema::ProviderCapabilities;
use async_trait::async_trait;
use futures::stream::Stream;
use rmcp::model::ContentBlock;
use std::pin::Pin;
use systemprompt_wire::canonical::CanonicalEvent;

pub use super::provider_params::{
    GenerationParams, SchemaGenerationParams, SearchGenerationParams, StructuredGenerationParams,
    ToolGenerationParams, ToolResultsParams,
};
pub use super::served_model::{catalog_default_model, catalog_pricing, catalog_supports_model};
pub use systemprompt_manifest::services::ai::ModelPricing;

/// The canonical event stream a provider yields for one streamed turn.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<CanonicalEvent>> + Send>>;

// Why: Native async trait methods are not dyn-compatible; boxed providers
// require `async_trait`.
#[async_trait]
pub trait ProviderClient: Send + Sync {
    fn name(&self) -> &str;

    fn capabilities(&self) -> ProviderCapabilities;

    fn supports_model(&self, model: &str) -> bool;

    fn supports_sampling(&self, sampling: Option<&SamplingParams>) -> bool;

    fn default_model(&self) -> &str;

    fn get_pricing(&self, model: &str) -> Option<ModelPricing>;

    // Why: health is what the resilience layer has observed — an open circuit
    // means recent calls failed — not an assumption that a registered
    // provider is reachable; a bare provider has no observation and reports
    // itself available.
    fn is_available(&self) -> bool {
        true
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse>;

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)>;

    async fn generate_with_tool_results(
        &self,
        params: ToolResultsParams<'_>,
    ) -> Result<AiResponse> {
        let mut messages = params.base.messages.to_vec();

        let mut tool_summary = String::new();
        for (call, result) in params.tool_calls.iter().zip(params.tool_results.iter()) {
            let content_text: String = result
                .content
                .iter()
                .filter_map(|c| match c {
                    ContentBlock::Text(text_content) => Some(text_content.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            if result.is_error.unwrap_or(false) {
                tool_summary.push_str(&format!("Tool {} failed: {}\n", call.name, content_text));
            } else {
                tool_summary.push_str(&format!("Tool {} result: {}\n", call.name, content_text));
            }
        }

        messages.push(AiMessage {
            role: crate::models::ai::MessageRole::User,
            content: format!(
                "Based on the tool results above, please provide a helpful response to the \
                 original question:\n\n{tool_summary}"
            ),
            parts: Vec::new(),
        });

        let gen_params = GenerationParams {
            messages: &messages,
            model: params.base.model,
            max_output_tokens: params.base.max_output_tokens,
            sampling: params.base.sampling,
            reasoning: params.base.reasoning,
        };
        self.generate(gen_params).await
    }

    async fn generate_structured(
        &self,
        params: StructuredGenerationParams<'_>,
    ) -> Result<AiResponse> {
        self.generate(params.base).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse>;

    fn supports_json_mode(&self) -> bool {
        false
    }

    fn supports_structured_output(&self) -> bool {
        false
    }

    async fn generate_stream(&self, _params: GenerationParams<'_>) -> Result<EventStream> {
        Err(AiError::capability_unsupported(
            self.name(),
            ProviderCapability::Streaming,
        ))
    }

    async fn generate_with_tools_stream(
        &self,
        _params: ToolGenerationParams<'_>,
    ) -> Result<EventStream> {
        Err(AiError::capability_unsupported(
            self.name(),
            ProviderCapability::ToolStreaming,
        ))
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn supports_google_search(&self) -> bool {
        false
    }

    async fn generate_with_google_search(
        &self,
        _params: SearchGenerationParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        Err(AiError::capability_unsupported(
            self.name(),
            ProviderCapability::GoogleSearch,
        ))
    }

    fn supports_embeddings(&self) -> bool {
        false
    }

    async fn embed(&self, _request: &EmbeddingRequest) -> Result<EmbeddingResponse> {
        Err(AiError::capability_unsupported(
            self.name(),
            ProviderCapability::Embeddings,
        ))
    }
}
