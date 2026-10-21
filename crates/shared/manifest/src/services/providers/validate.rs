//! Connectivity validation of the provider registry: unique provider names,
//! SSRF-guarded endpoints, credentials and globally-unique model ids/aliases.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_wire::hosting::names_a_project_literally;

use super::{ProviderRegistry, ProviderRegistryError, ProviderRegistryResult, headers};

impl ProviderRegistry {
    pub fn validate(&self) -> ProviderRegistryResult<()> {
        let trusted = systemprompt_models::net::trusted_http_hosts_from_env();
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
            systemprompt_models::net::validate_outbound_url_with_trust(
                &provider.endpoint,
                &trusted,
            )
            .map_err(|source| ProviderRegistryError::BlockedEndpoint {
                provider: provider.name.as_str().to_owned(),
                endpoint: provider.endpoint.clone(),
                source,
            })?;
            if let Some(header) = headers::reserved_name(provider.extra_headers.keys()) {
                return Err(ProviderRegistryError::ReservedExtraHeader {
                    provider: provider.name.as_str().to_owned(),
                    header: header.to_owned(),
                });
            }
            if provider.credential.is_platform() && provider.api_key_secret.is_none() {
                return Err(ProviderRegistryError::MissingApiKeySecret {
                    provider: provider.name.as_str().to_owned(),
                });
            }
            if let Some(header) = headers::reserved_name(provider.forward_headers.iter()) {
                return Err(ProviderRegistryError::ReservedExtraHeader {
                    provider: provider.name.as_str().to_owned(),
                    header: header.to_owned(),
                });
            }
            if names_a_project_literally(&provider.endpoint) {
                return Err(ProviderRegistryError::LiteralProjectInEndpoint {
                    provider: provider.name.as_str().to_owned(),
                    endpoint: provider.endpoint.clone(),
                });
            }

            for model in &provider.models {
                if model.id.as_str().is_empty() {
                    return Err(ProviderRegistryError::EmptyModelId {
                        provider: provider.name.clone(),
                    });
                }
                if !seen_models.insert(model.id.as_str()) {
                    return Err(ProviderRegistryError::DuplicateModel {
                        id: model.id.clone(),
                    });
                }
                for alias in &model.aliases {
                    if !seen_models.insert(alias.as_str()) {
                        return Err(ProviderRegistryError::DuplicateModel { id: alias.clone() });
                    }
                }
            }
        }
        Ok(())
    }
}
