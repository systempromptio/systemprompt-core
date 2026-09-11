//! The Vertex AI rate card: what a Vertex model costs, and whether we serve it.
//!
//! Vertex's publisher listing is Google's global Model Garden catalog. It says
//! nothing about modality — a chat model, an embedding model and a text-to-
//! speech model are the same JSON shape — and nothing about entitlement, which
//! is only proven by calling. So boot-time discovery cannot decide what to
//! publish on its own.
//!
//! This card is that decision, and it is one file rather than two because the
//! two questions have the same answer: the gateway refuses to dispatch to a
//! model it cannot price, so "priced here" and "allowed here" are necessarily
//! the same set. A listed model with no entry is reported as unpriced and left
//! unpublished; an entry Vertex stops listing is reported rather than deleted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId};

use super::{ProviderModel, ProviderRegistryError, ProviderRegistryResult};
use crate::services::ai::{ModelCapabilities, ModelLimits, ModelPricing};

const VERTEX_RATE_CARD_YAML: &str = include_str!("vertex_rate_card.yaml");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexRateCardEntry {
    /// The model as Vertex names it: `{publisher}/{model}`, taken from the
    /// listing's `publishers/{publisher}/models/{model}`.
    pub upstream: String,

    pub provider: ProviderId,

    pub id: ModelId,

    /// What our wire puts on the request — the bare name on the gemini wire,
    /// the publisher-qualified name on the MaaS openai-chat surface.
    pub upstream_model: String,

    /// Publish this model even when Vertex marks it preview/experimental.
    #[serde(default)]
    pub allow_preview: bool,

    #[serde(default)]
    pub pricing: ModelPricing,

    #[serde(default)]
    pub capabilities: ModelCapabilities,

    #[serde(default)]
    pub limits: ModelLimits,

    #[serde(default)]
    pub aliases: Vec<ModelId>,
}

impl VertexRateCardEntry {
    /// The publisher segment of [`Self::upstream`] (`qwen`, `google`, …).
    #[must_use]
    pub fn publisher(&self) -> &str {
        self.upstream.split('/').next().unwrap_or(&self.upstream)
    }

    /// The registry model this entry publishes.
    ///
    /// Governance is left unset so the model inherits its provider's posture:
    /// a discovered model must never quietly claim a weaker guarantee than the
    /// entry that carries it.
    #[must_use]
    pub fn to_provider_model(&self) -> ProviderModel {
        ProviderModel {
            id: self.id.clone(),
            aliases: self.aliases.clone(),
            upstream_model: Some(self.upstream_model.clone()),
            pricing: self.pricing,
            capabilities: self.capabilities,
            limits: self.limits,
            governance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexRateCard {
    pub entries: Vec<VertexRateCardEntry>,
}

impl VertexRateCard {
    /// The card compiled into the binary.
    pub fn embedded() -> ProviderRegistryResult<Self> {
        serde_yaml::from_str(VERTEX_RATE_CARD_YAML)
            .map_err(|e| ProviderRegistryError::InvalidVertexRateCard(e.to_string()))
    }

    #[must_use]
    pub fn lookup(&self, upstream: &str) -> Option<&VertexRateCardEntry> {
        self.entries.iter().find(|e| e.upstream == upstream)
    }

    /// Every entry priced for one provider.
    pub fn entries_for<'a>(
        &'a self,
        provider: &'a str,
    ) -> impl Iterator<Item = &'a VertexRateCardEntry> {
        self.entries
            .iter()
            .filter(move |e| e.provider.as_str() == provider)
    }

    /// The publishers a provider must list to see everything it prices, in
    /// first-appearance order and without repeats.
    #[must_use]
    pub fn publishers_for(&self, provider: &str) -> Vec<String> {
        let mut publishers: Vec<String> = Vec::new();
        for entry in self.entries_for(provider) {
            let publisher = entry.publisher();
            if !publishers.iter().any(|p| p == publisher) {
                publishers.push(publisher.to_owned());
            }
        }
        publishers
    }
}
