use anyhow::Result;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use uuid::Uuid;

use crate::models::ai::{AiRequest, GoogleSearchParams, SearchGroundedResponse};
use crate::services::providers::{GenerationParams, SearchGenerationParams, ToolGenerationParams};

use super::service::AiService;
use super::stream_wrapper::{StreamStorageParams, StreamStorageWrapper};

impl AiService {
    pub async fn generate_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_id = Uuid::new_v4();
        let start = std::time::Instant::now();
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

        let inner_stream = provider.generate_stream(params).await?;

        let wrapped_stream = StreamStorageWrapper::new(StreamStorageParams {
            inner: inner_stream,
            storage: self.storage.clone(),
            request: request.clone(),
            request_id,
            start,
            provider: request.provider().to_string(),
            model: request.model().to_string(),
        });

        Ok(Box::pin(wrapped_stream))
    }

    pub async fn generate_with_tools_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_id = Uuid::new_v4();
        let start = std::time::Instant::now();
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

        let inner_stream = provider.generate_with_tools_stream(params).await?;

        let wrapped_stream = StreamStorageWrapper::new(StreamStorageParams {
            inner: inner_stream,
            storage: self.storage.clone(),
            request: request.clone(),
            request_id,
            start,
            provider: request.provider().to_string(),
            model: request.model().to_string(),
        });

        Ok(Box::pin(wrapped_stream))
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
        let model = params
            .model
            .or_else(|| {
                let cfg = self.default_model();
                (!cfg.is_empty()).then_some(cfg)
            })
            .unwrap_or_else(|| provider.default_model());
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
