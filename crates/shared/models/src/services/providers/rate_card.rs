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
//! `RETIREMENT_NOTICE_DAYS` is how close to retirement a model may be and
//! still be published: a model retiring inside the window is withheld from
//! discovery so a developer who picks it today is not cut off mid-project,
//! while an explicit catalog declaration is the operator's call and is kept
//! with a warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId};

use super::{ProviderModel, ProviderRegistryError, ProviderRegistryResult};
use crate::services::ai::{ModelCapabilities, ModelLimits, ModelPricing};

const VERTEX_RATE_CARD_YAML: &str = include_str!("vertex_rate_card.yaml");

pub const RETIREMENT_NOTICE_DAYS: u64 = 30;

/// The launch stage as Google's model page states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentedLaunchStage {
    Ga,
    Preview,
}

/// One model on the Vertex rate card, every field read from the official
/// documentation page named in `docs`.
///
/// `upstream` is the listing name (`{publisher}/{model}`); `upstream_model` is
/// what the wire sends — bare on the gemini wire, publisher-qualified on the
/// `MaaS` openai-chat surface. `launch_stage` and `released` are the stage and
/// release date the page states (`released` is absent when only a deprecation
/// notice still names the model); `allow_preview` publishes a model the page
/// marks preview or experimental. `retires_on` is the announced retirement
/// date and `price_until` the last day the recorded price applies when an
/// introductory price has an announced end — the scheduler warns ahead of both.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexRateCardEntry {
    pub upstream: String,

    pub provider: ProviderId,

    pub id: ModelId,

    pub upstream_model: String,

    #[serde(default)]
    pub allow_preview: bool,

    pub launch_stage: DocumentedLaunchStage,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released: Option<NaiveDate>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retires_on: Option<NaiveDate>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_until: Option<NaiveDate>,

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
    #[must_use]
    pub fn publisher(&self) -> &str {
        self.upstream.split('/').next().unwrap_or(&self.upstream)
    }

    #[must_use]
    pub fn is_supported(&self, today: NaiveDate) -> bool {
        let stage_ok = self.launch_stage == DocumentedLaunchStage::Ga || self.allow_preview;
        stage_ok && !self.is_retiring(today)
    }

    #[must_use]
    pub fn is_retiring(&self, today: NaiveDate) -> bool {
        let horizon = today
            .checked_add_days(Days::new(RETIREMENT_NOTICE_DAYS))
            .unwrap_or(today);
        self.retires_on.is_some_and(|retires| retires <= horizon)
    }

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
    pub fn embedded() -> ProviderRegistryResult<Self> {
        let card: Self = serde_yaml::from_str(VERTEX_RATE_CARD_YAML)
            .map_err(|e| ProviderRegistryError::InvalidVertexRateCard(e.to_string()))?;
        card.validate()?;
        Ok(card)
    }

    pub fn validate(&self) -> ProviderRegistryResult<()> {
        self.entries
            .iter()
            .try_for_each(VertexRateCardEntry::validate)
    }

    #[must_use]
    pub fn lookup(&self, upstream: &str) -> Option<&VertexRateCardEntry> {
        self.entries.iter().find(|e| e.upstream == upstream)
    }

    #[must_use]
    pub fn lookup_id(&self, id: &str) -> Option<&VertexRateCardEntry> {
        self.entries.iter().find(|e| e.id.as_str() == id)
    }

    pub fn entries_for<'a>(
        &'a self,
        provider: &'a str,
    ) -> impl Iterator<Item = &'a VertexRateCardEntry> {
        self.entries
            .iter()
            .filter(move |e| e.provider.as_str() == provider)
    }

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
