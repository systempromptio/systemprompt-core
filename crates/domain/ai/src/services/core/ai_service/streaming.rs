use anyhow::Result;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;

use crate::models::ai::{AiRequest, GoogleSearchParams, SearchGroundedResponse};
use crate::services::providers::{GenerationParams, SearchGenerationParams, ToolGenerationParams};

use super::service::AiService;

impl AiService {
    pub async fn generate_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let provider = self.get_provider(request.provider())?;
        if !provider.supports_streaming() {
            return Err(anyhow::anyhow!(
                "Provider {} does not support streaming",
                request.provider()
            ));
        }
        let mut params = GenerationParams::new(
            &request.messages,
            request.model(),
            request.max_output_tokens(),
        );
        if let Some(sampling) = request.sampling.as_ref() {
            params = params.with_sampling(sampling);
        }
        provider.generate_stream(params).await
    }

    pub async fn generate_with_tools_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let provider = self.get_provider(request.provider())?;
        if !provider.supports_streaming() {
            return Err(anyhow::anyhow!(
                "Provider {} does not support streaming",
                request.provider()
            ));
        }
        let tools = request.tools.clone().unwrap_or_else(Vec::new);
        let mut base = GenerationParams::new(
            &request.messages,
            request.model(),
            request.max_output_tokens(),
        );
        if let Some(sampling) = request.sampling.as_ref() {
            base = base.with_sampling(sampling);
        }
        let params = ToolGenerationParams::new(base, tools);
        provider.generate_with_tools_stream(params).await
    }

    pub async fn generate_with_google_search(
        &self,
        params: GoogleSearchParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        let provider = self
            .providers
            .values()
            .find(|p| p.supports_google_search())
            .ok_or_else(|| anyhow::anyhow!("No provider with Google Search support available"))?;
        let model = params.model.unwrap_or_else(|| provider.default_model());
        let mut base = GenerationParams::new(&params.messages, model, params.max_output_tokens);
        if let Some(sampling) = params.sampling.as_ref() {
            base = base.with_sampling(sampling);
        }
        let mut search_params = SearchGenerationParams::new(base);
        if let Some(urls) = params.urls {
            search_params = search_params.with_urls(urls);
        }
        if let Some(schema) = params.response_schema {
            search_params = search_params.with_response_schema(schema);
        }
        provider.generate_with_google_search(search_params).await
    }

    pub async fn health_check(&self) -> Result<HashMap<String, bool>> {
        let mut health = HashMap::new();
        for name in self.providers.keys() {
            health.insert(format!("provider_{name}"), true);
        }
        let tool_health = self.tool_provider.health_check().await?;
        for (service_id, is_healthy) in tool_health {
            health.insert(format!("tool_{service_id}"), is_healthy);
        }
        Ok(health)
    }
}
