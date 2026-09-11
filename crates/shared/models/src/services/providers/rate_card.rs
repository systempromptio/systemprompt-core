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
//! Every entry also carries its lifecycle as Google's documentation states it
//! — launch stage, release date, retirement date, and the page it was read
//! from — because "currently supported" is a documentation fact, not a
//! listing fact: Vertex keeps listing a model right up to the day it is
//! switched off. [`VertexRateCardEntry::is_supported`] is the rule, and it is
//! evaluated against a date so that it can be tested and so that boot never
//! calls a model to find out whether it still exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId};

use super::{ProviderModel, ProviderRegistryError, ProviderRegistryResult};
use crate::services::ai::{ModelCapabilities, ModelLimits, ModelPricing};

const VERTEX_RATE_CARD_YAML: &str = include_str!("vertex_rate_card.yaml");

/// How close to its retirement a model may be and still be published.
///
/// A model retiring inside this window is withheld from discovery so that a
/// developer who picks it today is not cut off mid-project; an explicit
/// catalog declaration is the operator's call and is kept, with a warning.
pub const RETIREMENT_NOTICE_DAYS: u64 = 30;

/// The launch stage as Google's model page states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentedLaunchStage {
    Ga,
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexRateCardEntry {
    /// The model as Vertex names it: `{publisher}/{model}`, taken from the
    /// listing's `publishers/{publisher}/models/{model}`.
    pub upstream: String,

    pub provider: ProviderId,

    pub id: ModelId,

    /// What our wire puts on the request — the bare name on the gemini wire,
    /// the publisher-qualified name on the `MaaS` openai-chat surface.
    pub upstream_model: String,

    /// Publish this model even when Vertex (or the documentation) marks it
    /// preview or experimental.
    #[serde(default)]
    pub allow_preview: bool,

    /// The stage Google's documentation gives the model.
    pub launch_stage: DocumentedLaunchStage,

    /// The release date Google's documentation gives the model. Absent when
    /// the only page still naming the model is a deprecation notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released: Option<NaiveDate>,

    /// The retirement date Google has announced, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retires_on: Option<NaiveDate>,

    /// The last day the recorded price applies, when Google has announced a
    /// change (introductory pricing). The scheduler warns ahead of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_until: Option<NaiveDate>,

    /// The official documentation page every field above was read from.
    pub docs: String,

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

    /// Whether the documentation says this model may be served on `today`:
    /// GA (or an explicit preview opt-in), and not retiring within
    /// [`RETIREMENT_NOTICE_DAYS`].
    #[must_use]
    pub fn is_supported(&self, today: NaiveDate) -> bool {
        let stage_ok = self.launch_stage == DocumentedLaunchStage::Ga || self.allow_preview;
        stage_ok && !self.is_retiring(today)
    }

    /// Whether the model retires on or before `today` plus the notice window.
    #[must_use]
    pub fn is_retiring(&self, today: NaiveDate) -> bool {
        let horizon = today
            .checked_add_days(Days::new(RETIREMENT_NOTICE_DAYS))
            .unwrap_or(today);
        self.retires_on.is_some_and(|retires| retires <= horizon)
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

    fn validate(&self) -> ProviderRegistryResult<()> {
        let id = self.id.as_str();
        if !self.docs.starts_with("https://") {
            return Err(ProviderRegistryError::InvalidVertexRateCard(format!(
                "{id}: `docs` must be the official documentation URL"
            )));
        }
        let Some(released) = self.released else {
            return Ok(());
        };
        if self.retires_on.is_some_and(|retires| retires <= released) {
            return Err(ProviderRegistryError::InvalidVertexRateCard(format!(
                "{id}: `retires_on` is not after `released`"
            )));
        }
        if self.price_until.is_some_and(|until| until <= released) {
            return Err(ProviderRegistryError::InvalidVertexRateCard(format!(
                "{id}: `price_until` is not after `released`"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexRateCard {
    pub entries: Vec<VertexRateCardEntry>,
}

impl VertexRateCard {
    /// The card compiled into the binary, validated.
    pub fn embedded() -> ProviderRegistryResult<Self> {
        let card: Self = serde_yaml::from_str(VERTEX_RATE_CARD_YAML)
            .map_err(|e| ProviderRegistryError::InvalidVertexRateCard(e.to_string()))?;
        card.validate()?;
        Ok(card)
    }

    /// Every entry's lifecycle fields are coherent and sourced.
    pub fn validate(&self) -> ProviderRegistryResult<()> {
        self.entries
            .iter()
            .try_for_each(VertexRateCardEntry::validate)
    }

    #[must_use]
    pub fn lookup(&self, upstream: &str) -> Option<&VertexRateCardEntry> {
        self.entries.iter().find(|e| e.upstream == upstream)
    }

    /// The entry published under a model id, if any.
    #[must_use]
    pub fn lookup_id(&self, id: &str) -> Option<&VertexRateCardEntry> {
        self.entries.iter().find(|e| e.id.as_str() == id)
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
