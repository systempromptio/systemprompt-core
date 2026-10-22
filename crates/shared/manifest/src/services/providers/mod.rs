//! Provider registry: the single source of upstream connectivity.
//!
//! [`ProviderRegistry`] is the `providers:` list of the services tree
//! (`services/ai/providers.yaml` by convention, merged across includes). It is
//! implementation configuration shipped with the deployment, not a
//! per-environment profile section: the same catalog boots every environment
//! and only the credentials it names differ. Each [`ProviderEntry`] declares
//! one upstream exactly once — its
//! [`WireProtocol`], endpoint, credential ([`SecretName`]), extra headers, and
//! the model catalog it serves. The two policy layers reference entries by
//! [`ProviderId`] and never re-declare connectivity: the gateway policy
//! (`services.gateway`) routes external model names to a provider, and the AI
//! policy (`services/ai/config.yaml`) selects an agent default and per-provider
//! overrides.
//!
//! Validation here is the authority for connectivity: unique provider names,
//! SSRF-guarded endpoints, and globally-unique model ids/aliases. The gateway
//! and AI layers validate only their references *into* this registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod credential;
mod discovery_report;
mod error;
mod headers;
mod rate_card;
mod validate;

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};

use crate::services::ai::{ModelCapabilities, ModelGovernance, ModelPricing};
use systemprompt_models::providers::{ApiSurface, without_context_variant};
use systemprompt_wire::anthropic::AnthropicBeta;
use systemprompt_wire::{Hosting, ModelLimits, WireProtocol};

pub use credential::ProviderCredentialSource;
pub use discovery_report::DiscoveryReport;
pub use error::{ProviderRegistryError, ProviderRegistryResult, VertexRateCardDefect};
pub use rate_card::{
    DocumentedLaunchStage, RETIREMENT_NOTICE_DAYS, VertexRateCard, VertexRateCardEntry,
};

const DEFAULT_CATALOG_YAML: &str = include_str!("default_catalog.yaml");

#[derive(Deserialize)]
struct DefaultCatalogFile {
    providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderModel {
    pub id: ModelId,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<ModelId>,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,

    #[serde(default)]
    pub pricing: ModelPricing,

    #[serde(default)]
    pub capabilities: ModelCapabilities,

    #[serde(default)]
    pub limits: ModelLimits,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<ModelGovernance>,
}

impl ProviderModel {
    #[must_use]
    pub fn matches(&self, requested: &str) -> bool {
        let requested = without_context_variant(requested);
        self.id.as_str() == requested || self.aliases.iter().any(|a| a.as_str() == requested)
    }

    #[must_use]
    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model
            .as_deref()
            .unwrap_or_else(|| without_context_variant(requested))
    }
}

#[must_use]
pub const fn surface_for(protocol: WireProtocol) -> ApiSurface {
    match protocol {
        WireProtocol::Anthropic => ApiSurface::Anthropic,
        WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses => ApiSurface::OpenAi,
        WireProtocol::Gemini => ApiSurface::Gemini,
    }
}

#[must_use]
pub fn upstream_model_in<'a>(models: &'a [ProviderModel], requested: &'a str) -> &'a str {
    models.iter().find(|m| m.matches(requested)).map_or_else(
        || without_context_variant(requested),
        |model| model.effective_upstream_model(requested),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderEntry {
    pub name: ProviderId,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub wire: WireProtocol,

    pub surface: ApiSurface,

    pub endpoint: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_secret: Option<SecretName>,

    #[serde(default, skip_serializing_if = "ProviderCredentialSource::is_platform")]
    pub credential: ProviderCredentialSource,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_from: Option<ProviderId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forward_headers: Vec<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_betas: Option<BTreeSet<AnthropicBeta>>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ProviderModel>,

    #[serde(default)]
    pub governance: ModelGovernance,
}

impl ProviderEntry {
    #[must_use]
    pub const fn is_user_credential(&self) -> bool {
        matches!(self.credential, ProviderCredentialSource::User)
    }

    // Why: one subscription can span several entries (OpenCode Go serves
    // Chat Completions and Anthropic Messages from separate endpoints); the
    // member stores the key once, under the entry `credential_from` names.
    #[must_use]
    pub fn credential_owner(&self) -> &str {
        self.credential_from
            .as_ref()
            .map_or(self.name.as_str(), ProviderId::as_str)
    }

    #[must_use]
    pub const fn takes_own_personal_key(&self) -> bool {
        self.is_user_credential() && self.credential_from.is_none()
    }

    #[must_use]
    pub const fn platform_secret(&self) -> Option<&SecretName> {
        match self.credential {
            ProviderCredentialSource::Platform => self.api_key_secret.as_ref(),
            ProviderCredentialSource::User => None,
        }
    }

    #[must_use]
    pub fn forwards_header(&self, name: &str) -> bool {
        self.forward_headers
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(name))
    }

    #[must_use]
    pub fn find_model(&self, requested: &str) -> Option<&ProviderModel> {
        self.models.iter().find(|m| m.matches(requested))
    }

    // Why: a failover re-sends the requested model to another host of the
    // same vendor model (Claude on Vertex AI to Anthropic's API), where the
    // catalog id must differ because ids are unique across the registry. The
    // upstream name is what the two entries share.
    #[must_use]
    pub fn find_served_model(&self, requested: &str) -> Option<&ProviderModel> {
        self.find_model(requested).or_else(|| {
            let bare = without_context_variant(requested);
            self.models
                .iter()
                .find(|m| m.upstream_model.as_deref() == Some(bare))
        })
    }

    #[must_use]
    pub fn upstream_model_for<'a>(
        &'a self,
        route_override: Option<&'a str>,
        requested: &'a str,
    ) -> &'a str {
        upstream_model_in(&self.models, route_override.unwrap_or(requested))
    }

    #[must_use]
    pub fn hosting(&self) -> Hosting {
        Hosting::of(&self.endpoint)
    }

    #[must_use]
    pub fn effective_governance(&self, requested: &str) -> ModelGovernance {
        self.find_served_model(requested)
            .and_then(|m| m.governance)
            .unwrap_or(self.governance)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ProviderRegistry {
    pub providers: Vec<ProviderEntry>,
}

impl ProviderRegistry {
    pub fn default_seed() -> ProviderRegistryResult<Self> {
        let file: DefaultCatalogFile = serde_yaml::from_str(DEFAULT_CATALOG_YAML)
            .map_err(ProviderRegistryError::InvalidDefaultCatalog)?;
        Ok(Self {
            providers: file.providers,
        })
    }

    #[must_use]
    pub fn find_provider(&self, name: &str) -> Option<&ProviderEntry> {
        self.providers.iter().find(|p| p.name.as_str() == name)
    }

    #[must_use]
    pub fn contains_model(&self, requested: &str) -> bool {
        self.providers
            .iter()
            .any(|p| p.find_model(requested).is_some())
    }

    #[must_use]
    pub fn is_hidden_model(&self, requested: &str) -> bool {
        self.providers
            .iter()
            .filter_map(|p| p.find_model(requested))
            .any(|m| m.hidden)
    }

    pub fn advertised_providers(&self) -> impl Iterator<Item = &ProviderEntry> {
        self.providers
            .iter()
            .filter(|entry| entry.surface.is_advertised())
    }

    #[must_use]
    pub fn advertised_model_ids(&self, surfaces: &[ApiSurface]) -> Vec<String> {
        self.advertised_providers()
            .filter(|entry| surfaces.is_empty() || surfaces.contains(&entry.surface))
            .flat_map(|entry| entry.models.iter())
            .filter(|model| !model.hidden)
            .map(|model| model.id.as_str().to_owned())
            .collect()
    }
}
