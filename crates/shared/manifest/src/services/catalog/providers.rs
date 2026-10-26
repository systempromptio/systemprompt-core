//! `services/ai/providers.yaml`: the vendor accounts.
//!
//! A provider is one vendor account: the credential it authenticates with,
//! the headers and user agent every request to it carries, its data
//! governance posture, the capability defaults its models start from, and the
//! endpoints it serves. `resilience` is the outbound timeout/retry/breaker
//! default for every endpoint (an endpoint may override it), and
//! `google_search_enabled` turns on Gemini's search grounding, so it needs a
//! `gemini`-wire endpoint. `enabled: false` takes every endpoint of the
//! provider out of every chain, everywhere.
//!
//! `user_agent` and `extra_headers` values may name `{install}`, expanded
//! from the profile by [`CatalogContext::expand`] when the catalog is
//! resolved, so installs never copy their own name into each string.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CredentialId, EndpointId, ProviderId};

use super::endpoint::EndpointConfig;
use crate::services::ai::{ModelCapabilities, ModelGovernance, ResilienceSettings};

const INSTALL_PLACEHOLDER: &str = "{install}";

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub credential: CredentialId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forward_headers: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
    #[serde(default)]
    pub governance: ModelGovernance,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub resilience: ResilienceSettings,
    #[serde(default)]
    pub google_search_enabled: bool,
    pub endpoints: BTreeMap<EndpointId, EndpointConfig>,
}

impl ProviderConfig {
    #[must_use]
    pub fn new(credential: impl Into<String>) -> Self {
        Self {
            credential: CredentialId::new(credential),
            display_name: None,
            description: None,
            user_agent: None,
            forward_headers: Vec::new(),
            extra_headers: HashMap::new(),
            governance: ModelGovernance::default(),
            enabled: true,
            capabilities: ModelCapabilities::default(),
            resilience: ResilienceSettings::default(),
            google_search_enabled: false,
            endpoints: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn endpoint(&self, id: &str) -> Option<&EndpointConfig> {
        self.endpoints.get(id)
    }

    #[must_use]
    pub fn forwards_header(&self, name: &str) -> bool {
        self.forward_headers
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ProviderRegistry {
    pub providers: BTreeMap<ProviderId, ProviderConfig>,
}

impl ProviderRegistry {
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProviderId, &ProviderConfig)> {
        self.providers.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

/// Profile facts the catalog's placeholders are expanded from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogContext {
    pub install: Option<String>,
}

impl CatalogContext {
    #[must_use]
    pub fn for_install(install: impl Into<String>) -> Self {
        Self {
            install: Some(install.into()),
        }
    }

    #[must_use]
    pub fn expand(&self, value: &str) -> String {
        self.install.as_deref().map_or_else(
            || value.to_owned(),
            |install| value.replace(INSTALL_PLACEHOLDER, install),
        )
    }
}
