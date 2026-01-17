use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::models::ai::{AiResponse, SamplingParams};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, ToolGenerationParams,
};
use crate::services::schema::ProviderCapabilities;

use super::provider::AnthropicProvider;
use super::{converters, generation};

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
        )
    }

    fn supports_sampling(&self, _sampling: Option<&SamplingParams>) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "claude-3-sonnet-20240229"
    }

    fn get_pricing(&self, model: &str) -> ModelPricing {
        match model {
            "claude-3-opus-20240229" => ModelPricing::new(0.015, 0.075),
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
        self.create_stream_request(params.base, Some(anthropic_tools)).await
    }
}
