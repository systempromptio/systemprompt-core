//! `AiProvider` implementation for Gemini.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::models::ai::{AiResponse, SamplingParams, SearchGroundedResponse, StreamChunk};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    ToolGenerationParams, ToolResultsParams, catalog_default_model, catalog_pricing,
    catalog_supports_model,
};
use crate::services::schema::ProviderCapabilities;

use super::provider::GeminiProvider;
use super::{generation, search, streaming, tools};

#[async_trait]
impl AiProvider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::gemini()
    }

    fn supports_model(&self, model: &str) -> bool {
        catalog_supports_model(&self.models, model)
    }

    fn supports_sampling(&self, _sampling: Option<&SamplingParams>) -> bool {
        true
    }

    fn default_model(&self) -> &str {
        catalog_default_model(&self.models, self.default_model_override.as_deref())
    }

    fn get_pricing(&self, model: &str) -> ModelPricing {
        catalog_pricing(&self.models, model)
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_google_search(&self) -> bool {
        self.google_search_enabled
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
        generation::generate(self, params).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        generation::generate_with_schema(self, params).await
    }

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        let mut builder = tools::ToolRequestParams::builder(
            params.base.messages,
            &params.tools,
            params.base.max_output_tokens,
            params.base.model,
        );
        if let Some(sampling) = params.base.sampling {
            builder = builder.with_sampling(sampling);
        }
        tools::generate_with_tools(self, builder.build()).await
    }

    async fn generate_with_tool_results(
        &self,
        params: ToolResultsParams<'_>,
    ) -> Result<AiResponse> {
        let mut builder = tools::ToolResultParams::builder(
            params.base.messages,
            params.tool_calls,
            params.tool_results,
            params.base.max_output_tokens,
            params.base.model,
        );
        if let Some(sampling) = params.base.sampling {
            builder = builder.with_sampling(sampling);
        }
        tools::generate_with_tool_results(self, builder.build()).await
    }

    async fn generate_stream(
        &self,
        params: GenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        streaming::generate_stream(self, params).await
    }

    async fn generate_with_google_search(
        &self,
        params: SearchGenerationParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        let mut builder = search::SearchParams::builder(
            params.base.messages,
            params.base.max_output_tokens,
            params.base.model,
        );
        if let Some(sampling) = params.base.sampling {
            builder = builder.with_sampling(sampling);
        }
        if let Some(urls) = params.urls {
            builder = builder.with_urls(urls);
        }
        search::generate_with_google_search(self, builder.build()).await
    }
}
