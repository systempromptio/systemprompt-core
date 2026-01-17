use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::models::ai::{AiResponse, SamplingParams};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, StructuredGenerationParams,
    ToolGenerationParams,
};
use crate::services::schema::ProviderCapabilities;

use super::provider::OpenAiProvider;
use super::{converters, generation};

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::openai()
    }

    fn supports_model(&self, model: &str) -> bool {
        matches!(
            model,
            "gpt-4-turbo"
                | "gpt-4"
                | "gpt-3.5-turbo"
                | "gpt-4o"
                | "gpt-4o-mini"
                | "o1"
                | "o1-mini"
                | "o1-preview"
                | "o3"
                | "o3-mini"
        )
    }

    fn supports_sampling(&self, _sampling: Option<&SamplingParams>) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "gpt-4-turbo"
    }

    fn get_pricing(&self, model: &str) -> ModelPricing {
        match model {
            "gpt-4" | "gpt-4-turbo" | "gpt-4-turbo-preview" => ModelPricing::new(0.01, 0.03),
            "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" => ModelPricing::new(0.00015, 0.0006),
            "gpt-3.5-turbo" | "gpt-3.5-turbo-0125" => ModelPricing::new(0.0005, 0.0015),
            "o1" | "o1-2024-12-17" => ModelPricing::new(0.015, 0.06),
            "o1-mini" | "o1-mini-2024-09-12" => ModelPricing::new(0.003, 0.012),
            _ => ModelPricing::new(0.0025, 0.01),
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

    async fn generate_structured(
        &self,
        params: StructuredGenerationParams<'_>,
    ) -> Result<AiResponse> {
        generation::generate_structured(self, params).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        generation::generate_with_schema(self, params).await
    }

    fn supports_json_mode(&self) -> bool {
        true
    }

    fn supports_structured_output(&self) -> bool {
        true
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
        let openai_tools = converters::convert_tools(params.tools)?;
        self.create_stream_request(params.base, Some(openai_tools))
            .await
    }
}
