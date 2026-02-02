use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::models::ai::{AiResponse, SamplingParams, SearchGroundedResponse};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    ToolGenerationParams,
};
use crate::services::schema::ProviderCapabilities;

use super::provider::AnthropicProvider;
use super::{converters, generation, search};

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::anthropic()
    }

    fn supports_model(&self, model: &str) -> bool {
        matches!(
            model,
            "claude-3-opus-20240229"
                | "claude-3-sonnet-20240229"
                | "claude-3-haiku-20240307"
                | "claude-3-5-sonnet-20241022"
                | "claude-3-5-haiku-20241022"
                | "claude-sonnet-4-20250514"
                | "claude-opus-4-20250514"
                | "claude-opus-4-5-20251101"
                | "claude-sonnet-4-5-20251101"
                | "claude-haiku-4-5-20251101"
        )
    }

    fn supports_sampling(&self, _sampling: Option<&SamplingParams>) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "claude-sonnet-4-20250514"
    }

    fn get_pricing(&self, model: &str) -> ModelPricing {
        match model {
            "claude-3-opus-20240229" | "claude-opus-4-20250514" => ModelPricing::new(0.015, 0.075),
            "claude-opus-4-5-20251101" => ModelPricing::new(0.005, 0.025),
            "claude-haiku-4-5-20251101" => ModelPricing::new(0.001, 0.005),
            "claude-3-5-haiku-20241022" => ModelPricing::new(0.0008, 0.004),
            "claude-3-haiku-20240307" => ModelPricing::new(0.00025, 0.00125),
            _ => ModelPricing::new(0.003, 0.015),
        }
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
        generation::generate(self, params).await
    }

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        generation::generate_with_tools(self, params).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        generation::generate_with_schema(self, params).await
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn generate_stream(
        &self,
        params: GenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        self.create_stream_request(params, None).await
    }

    async fn generate_with_tools_stream(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let anthropic_tools = converters::convert_tools(params.tools);
        self.create_stream_request(params.base, Some(anthropic_tools))
            .await
    }

    fn supports_google_search(&self) -> bool {
        self.web_search_enabled
    }

    async fn generate_with_google_search(
        &self,
        params: SearchGenerationParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        let search_params = search::SearchParams::new(
            params.base.messages,
            params.base.max_output_tokens,
            params.base.model,
        );
        let search_params = if let Some(sampling) = params.base.sampling {
            search_params.with_sampling(sampling)
        } else {
            search_params
        };
        search::generate_with_web_search(self, search_params).await
    }
}
