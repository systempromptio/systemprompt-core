use anyhow::Result;
use async_trait::async_trait;

use crate::models::ai::{AiResponse, SamplingParams};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, ToolGenerationParams,
};
use crate::services::schema::ProviderCapabilities;

use super::generation;
use super::provider::AnthropicProvider;

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
        generation::generate(
            self,
            params.messages,
            params.sampling,
            params.max_output_tokens,
            params.model,
        )
        .await
    }

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        generation::generate_with_tools(
            self,
            generation::ToolGenerationParams {
                messages: params.base.messages,
                tools: params.tools,
                sampling: params.base.sampling,
                max_output_tokens: params.base.max_output_tokens,
                model: params.base.model,
            },
        )
        .await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        generation::generate_with_schema(
            self,
            generation::SchemaGenerationParams {
                messages: params.base.messages,
                response_schema: params.response_schema,
                sampling: params.base.sampling,
                max_output_tokens: params.base.max_output_tokens,
                model: params.base.model,
            },
        )
        .await
    }
}
