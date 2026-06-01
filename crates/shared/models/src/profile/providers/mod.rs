//! Provider registry: the single source of upstream connectivity.
//!
//! [`ProviderRegistry`] is the per-environment `profile.providers` section.
//! Each [`ProviderEntry`] declares one upstream exactly once — its
//! [`WireProtocol`], endpoint, credential ([`SecretName`]), extra headers, and
//! the model catalog it serves. The two policy layers reference entries by
//! [`ProviderId`] and never re-declare connectivity: the gateway policy
//! (`profile.gateway`) routes external model names to a provider, and the AI
//! policy (`services/ai/config.yaml`) selects an agent default and per-provider
//! overrides.
//!
//! Validation here is the authority for connectivity: unique provider names,
//! SSRF-guarded endpoints, and globally-unique model ids/aliases. The gateway
//! and AI layers validate only their references *into* this registry.

mod error;
mod protocol;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};

use crate::services::ai::{ModelCapabilities, ModelLimits, ModelPricing};

pub use error::{ProviderRegistryError, ProviderRegistryResult};
pub use protocol::WireProtocol;

/// One model served by a provider: identity, routing, and economics.
///
/// A model's full description lives here exactly once: identity and routing
/// (id, aliases, `upstream_model`, pricing) alongside agent-side capabilities
/// and limits.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderModel {
    pub id: ModelId,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<ModelId>,

    /// Vendor-side model name to send upstream when it differs from
    /// [`Self::id`] (the external-facing name). `None` forwards `id`
    /// unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,

    #[serde(default)]
    pub pricing: ModelPricing,

    #[serde(default)]
    pub capabilities: ModelCapabilities,

    #[serde(default)]
    pub limits: ModelLimits,
}

impl ProviderModel {
    #[must_use]
    pub fn matches(&self, requested: &str) -> bool {
        self.id.as_str() == requested || self.aliases.iter().any(|a| a.as_str() == requested)
    }

    #[must_use]
    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model.as_deref().unwrap_or(requested)
    }
}

/// One upstream provider declared once: connectivity + the models it serves.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderEntry {
    pub name: ProviderId,

    pub protocol: WireProtocol,

    pub endpoint: String,

    pub api_key_secret: SecretName,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ProviderModel>,
}

impl ProviderEntry {
    #[must_use]
    pub fn find_model(&self, requested: &str) -> Option<&ProviderModel> {
        self.models.iter().find(|m| m.matches(requested))
    }
}

/// The `profile.providers` section: the registry of upstream providers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ProviderRegistry {
    pub providers: Vec<ProviderEntry>,
}

impl ProviderRegistry {
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

    pub fn validate(&self) -> ProviderRegistryResult<()> {
        let trusted = crate::net::trusted_http_hosts_from_env();
        let mut seen_providers: HashSet<&str> = HashSet::with_capacity(self.providers.len());
        let mut seen_models: HashSet<&str> = HashSet::new();

        for provider in &self.providers {
            if !seen_providers.insert(provider.name.as_str()) {
                return Err(ProviderRegistryError::DuplicateProvider {
                    name: provider.name.as_str().to_owned(),
                });
            }
            if provider.endpoint.is_empty() {
                return Err(ProviderRegistryError::EmptyEndpoint {
                    name: provider.name.as_str().to_owned(),
                });
            }
            crate::net::validate_outbound_url_with_trust(&provider.endpoint, &trusted).map_err(
                |e| ProviderRegistryError::BlockedEndpoint {
                    provider: provider.name.as_str().to_owned(),
                    endpoint: provider.endpoint.clone(),
                    reason: e.to_string(),
                },
            )?;

            for model in &provider.models {
                if model.id.as_str().is_empty() {
                    return Err(ProviderRegistryError::EmptyModelId {
                        id: provider.name.as_str().to_owned(),
                    });
                }
                if !seen_models.insert(model.id.as_str()) {
                    return Err(ProviderRegistryError::DuplicateModel {
                        id: model.id.as_str().to_owned(),
                    });
                }
                for alias in &model.aliases {
                    if !seen_models.insert(alias.as_str()) {
                        return Err(ProviderRegistryError::DuplicateModel {
                            id: alias.as_str().to_owned(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
