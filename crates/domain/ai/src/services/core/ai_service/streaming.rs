//! Streaming generation execution.
//!
//! One audited provider stream backs every streaming entry point:
//! [`AiService::generate_events_stream`] yields it whole, and the text
//! streams project it down to [`StreamChunk::Text`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{AiError, ProviderCapability, Result, RoutingError};
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::pin::Pin;
use systemprompt_identifiers::{AiRequestId, ModelId, ProviderId};

use crate::models::ai::{AiRequest, GoogleSearchParams, SearchGroundedResponse, StreamChunk};
use crate::services::providers::canonical_bridge::event_to_chunk;
use crate::services::providers::{
    EventStream, GenerationParams, ModelPricing, ProviderClient, SearchGenerationParams,
    ToolGenerationParams,
};

use super::service::AiService;
use super::stream_wrapper::{StreamStorageParams, StreamStorageWrapper};

type ChunkStream = Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>;

#[derive(Clone, Copy)]
enum Tooling {
    Withheld,
    Offered,
}

impl AiService {
    pub async fn generate_stream(&self, request: &AiRequest) -> Result<ChunkStream> {
        let events = self.audited_events(request, Tooling::Withheld).await?;
        Ok(text_chunks(events))
    }

    pub async fn generate_with_tools_stream(&self, request: &AiRequest) -> Result<ChunkStream> {
        let events = self.audited_events(request, Tooling::Offered).await?;
        Ok(text_chunks(events))
    }

    pub async fn generate_events_stream(&self, request: &AiRequest) -> Result<EventStream> {
        let tooling = if request.has_tools() {
            Tooling::Offered
        } else {
            Tooling::Withheld
        };
        self.audited_events(request, tooling).await
    }

    async fn audited_events(&self, request: &AiRequest, tooling: Tooling) -> Result<EventStream> {
        let request_id = AiRequestId::generate();
        let start = std::time::Instant::now();
        let provider = self.get_provider(request)?;

        if !provider.supports_streaming() {
            return Err(AiError::Routing(RoutingError::CapabilityUnsupported {
                provider: ProviderId::new(request.provider()),
                capability: ProviderCapability::Streaming,
            }));
        }

        let params = GenerationParams::from_request(request, request.model())?;
        let pricing = priced_model(provider.as_ref(), request.model())?;
        let inner = match tooling {
            Tooling::Withheld => provider.generate_stream(params).await?,
            Tooling::Offered => {
                let tools = request.tools.clone().unwrap_or_else(Vec::new);
                provider
                    .generate_with_tools_stream(ToolGenerationParams::new(params, tools))
                    .await?
            },
        };

        Ok(Box::pin(StreamStorageWrapper::new(StreamStorageParams {
            inner,
            storage: self.storage.clone(),
            audit_tasks: self.audit_tasks.clone(),
            request: request.clone(),
            request_id,
            start,
            provider: request.provider().to_owned(),
            model: request.model().to_owned(),
            pricing,
        })))
    }

    pub async fn generate_with_google_search(
        &self,
        params: GoogleSearchParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        let provider = self
            .clients
            .values()
            .find(|p| p.supports_google_search())
            .ok_or(AiError::Routing(RoutingError::NoProviderWithCapability {
                capability: ProviderCapability::GoogleSearch,
            }))?;
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
        for (endpoint, provider) in self.clients.iter() {
            health.insert(format!("provider_{endpoint}"), provider.is_available());
        }
        let tool_health = self.tool_provider.health_check().await?;
        for (service_id, is_healthy) in tool_health {
            health.insert(format!("tool_{service_id}"), is_healthy);
        }
        Ok(health)
    }
}

fn priced_model(provider: &dyn ProviderClient, model: &str) -> Result<ModelPricing> {
    provider.get_pricing(model).ok_or_else(|| {
        AiError::Routing(RoutingError::UnknownModel {
            provider: ProviderId::new(provider.name()),
            model: ModelId::new(model),
        })
    })
}

fn text_chunks(events: EventStream) -> ChunkStream {
    Box::pin(events.filter_map(|event| {
        futures::future::ready(match event {
            Ok(event) => event_to_chunk(event).map(Ok),
            Err(e) => Some(Err(e)),
        })
    }))
}
