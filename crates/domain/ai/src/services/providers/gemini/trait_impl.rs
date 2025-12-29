use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::models::ai::{AiResponse, SamplingParams, SearchGroundedResponse};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    ToolGenerationParams, ToolResultsParams,
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
        matches!(
            model,
            "gemini-2.5-flash-lite"
                | "gemini-2.5-flash"
                | "gemini-2.5-pro"
                | "gemini-3-pro-preview"
                | "gemini-2.0-flash"
                | "gemini-2.0-flash-lite"
                | "gemini-1.5-flash"
                | "gemini-1.5-flash-latest"
                | "gemini-1.5-flash-8b"
        )
    }

    fn supports_sampling(&self, _sampling: Option<&SamplingParams>) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "gemini-2.5-flash-lite"
    }

    fn get_pricing(&self, model: &str) -> ModelPricing {
        match model {
            "gemini-3-pro-preview" => ModelPricing::new(0.002, 0.012),
            "gemini-2.5-pro" | "gemini-2.5-pro-preview-05-06" => ModelPricing::new(0.00125, 0.01),
            "gemini-2.5-flash"
            | "gemini-2.5-flash-preview-04-17"
            | "gemini-2.5-flash-preview-09-2025" => ModelPricing::new(0.0003, 0.0025),
            "gemini-1.5-pro" => ModelPricing::new(0.00125, 0.005),
            "gemini-2.0-flash-lite"
            | "gemini-1.5-flash"
            | "gemini-1.5-flash-8b"
            | "gemini-1.5-flash-latest" => ModelPricing::new(0.000_075, 0.0003),
            _ => ModelPricing::new(0.0001, 0.0004),
        }
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_google_search(&self) -> bool {
        self.google_search_enabled
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
        generation::generate(
            self,
            params.messages,
            params.sampling,
            params.max_output_tokens,
            params.model,
        )
        .await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        let gen_params = generation::SchemaGenerationParams {
            messages: params.base.messages,
            response_schema: params.response_schema,
            sampling: params.base.sampling,
            max_output_tokens: params.base.max_output_tokens,
            model: params.base.model,
        };
        generation::generate_with_schema(self, gen_params).await
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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        streaming::generate_stream(
            self,
            params.messages,
            params.sampling,
            params.max_output_tokens,
            params.model,
        )
        .await
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
