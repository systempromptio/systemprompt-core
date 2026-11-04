//! `AiService` text embeddings: route to the provider that serves the model,
//! embed, and price the usage from the catalog.
//!
//! No `ai_requests` row is written for an embedding: the table's
//! `request_kind` check constraint admits only `turn`, `probe` and `utility`.
//! The priced usage is returned to the caller instead, and usage the
//! provider did not report arrives estimated rather than zero, so the cost
//! is never silently nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ProviderId;
use systemprompt_wire::canonical::CanonicalUsage;

use super::AiService;
use crate::error::{AiError, ProviderCapability, Result, RoutingError};
use crate::models::ai::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};

/// One embedding call's result together with what the catalog says it cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedded {
    pub response: EmbeddingResponse,
    pub provider: ProviderId,
    pub cost_microdollars: i64,
    pub latency_ms: u64,
}

impl AiService {
    pub async fn embed(&self, request: &EmbeddingRequest) -> Result<Embedded> {
        let client = self.provider_for(
            request.provider().map(ProviderId::as_str),
            request.model.as_str(),
        )?;
        if !client.supports_embeddings() {
            return Err(AiError::capability_unsupported(
                client.name(),
                ProviderCapability::Embeddings,
            ));
        }
        let pricing = client.get_pricing(request.model.as_str()).ok_or_else(|| {
            AiError::Routing(RoutingError::UnknownModel {
                provider: ProviderId::new(client.name()),
                model: request.model.clone(),
            })
        })?;
        let start = std::time::Instant::now();
        let response = client.embed(request).await?;
        let cost_microdollars = pricing.cost_microdollars(&canonical_usage(response.usage));
        tracing::info!(
            provider = client.name(),
            model = %request.model,
            inputs = request.inputs.len(),
            input_tokens = response.usage.input_tokens,
            estimated = response.usage.estimated,
            cost_microdollars,
            "embedding completed"
        );
        Ok(Embedded {
            response,
            provider: ProviderId::new(client.name()),
            cost_microdollars,
            latency_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

const fn canonical_usage(usage: EmbeddingUsage) -> CanonicalUsage {
    CanonicalUsage {
        input_tokens: usage.input_tokens,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        reasoning_tokens: 0,
        total_tokens: usage.total_tokens,
    }
}
