use crate::error::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::models::ai::{AiResponse, SamplingParams, SearchGroundedResponse, StreamChunk};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    ToolGenerationParams, catalog_default_model, catalog_pricing, catalog_supports_model,
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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        self.create_stream_request(params, None).await
    }

    async fn generate_with_tools_stream(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
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
