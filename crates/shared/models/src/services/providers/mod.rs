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

mod discovery_report;
mod error;
mod protocol;
mod rate_card;
mod surface;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};

use crate::services::ai::{ModelCapabilities, ModelGovernance, ModelLimits, ModelPricing};

pub use discovery_report::DiscoveryReport;
pub use error::{ProviderRegistryError, ProviderRegistryResult};
pub use protocol::WireProtocol;
pub use rate_card::{VertexRateCard, VertexRateCardEntry};
pub use surface::ApiSurface;

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
        self.id.as_str() == requested || self.aliases.iter().any(|a| a.as_str() == requested)
    }

    #[must_use]
    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model.as_deref().unwrap_or(requested)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderEntry {
    pub name: ProviderId,

    pub wire: WireProtocol,

    pub surface: ApiSurface,

    pub endpoint: String,

    pub api_key_secret: SecretName,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ProviderModel>,

    #[serde(default)]
    pub governance: ModelGovernance,
}

impl ProviderEntry {
    #[must_use]
    pub fn find_model(&self, requested: &str) -> Option<&ProviderModel> {
        self.models.iter().find(|m| m.matches(requested))
    }

    #[must_use]
    pub fn upstream_model_for<'a>(
        &'a self,
        route_override: Option<&'a str>,
        requested: &'a str,
    ) -> &'a str {
        route_override.unwrap_or_else(|| {
            self.find_model(requested)
                .map_or(requested, |model| model.effective_upstream_model(requested))
        })
    }

    #[must_use]
    pub fn effective_governance(&self, requested: &str) -> ModelGovernance {
        self.find_model(requested)
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
            .map_err(|e| ProviderRegistryError::InvalidDefaultCatalog(e.to_string()))?;
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

    pub fn advertised_providers(&self) -> impl Iterator<Item = &ProviderEntry> {
        self.providers
            .iter()
            .filter(|entry| entry.surface.is_advertised())
    }

    #[must_use]
    pub fn advertised_model_ids(&self, surfaces: &[ApiSurface]) -> Vec<String> {
        self.advertised_providers()
            .filter(|entry| surfaces.is_empty() || surfaces.contains(&entry.surface))
            .flat_map(|entry| {
                entry.models.iter().flat_map(|m| {
                    std::iter::once(m.id.as_str().to_owned())
                        .chain(m.aliases.iter().map(|a| a.as_str().to_owned()))
                })
            })
            .collect()
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
            if names_a_project_literally(&provider.endpoint) {
                return Err(ProviderRegistryError::LiteralProjectInEndpoint {
                    provider: provider.name.as_str().to_owned(),
                    endpoint: provider.endpoint.clone(),
                });
            }

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

pub const PROJECT_PLACEHOLDER: &str = "{project}";

// Why: a Google Cloud project id is a tenant identifier, and Vertex reports it
// verbatim in every IAM error it returns, which the gateway relays to the
// caller. A catalog that names one literally therefore ships that id to every
// installation of the image and every client that trips a 403. The id lives in
// exactly one place, the service-account key, and the endpoint says
// `{project}` instead.
#[must_use]
pub fn names_a_project_literally(endpoint: &str) -> bool {
    let Ok(url) = url::Url::parse(endpoint) else {
        return false;
    };
    let on_vertex = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("aiplatform.googleapis.com")
            || host
                .to_ascii_lowercase()
                .ends_with("-aiplatform.googleapis.com")
    });
    if !on_vertex {
        return false;
    }
    let mut segments = url.path_segments().into_iter().flatten();
    while let Some(segment) = segments.next() {
        if segment == "projects" {
            return segments
                .next()
                .is_some_and(|id| id != "%7Bproject%7D" && id != PROJECT_PLACEHOLDER);
        }
    }
    false
}
