//! Provider-registry mutations behind the `admin config catalog` surface.
//!
//! [`ProviderCatalogService`] edits the typed
//! [`ProviderRegistry`] on a profile: declaring or removing upstream providers
//! and the models each provider serves. Upserts replace an entry in place;
//! a provider upsert preserves the existing model catalog so connectivity can
//! be re-declared without re-listing models. Callers revalidate and persist
//! the profile after a successful mutation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::profile::{
    ApiSurface, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
};
use systemprompt_models::services::ai::{ModelCapabilities, ModelLimits, ModelPricing};

use crate::error::{ConfigError, ConfigResult};

#[derive(Debug, Clone)]
pub struct ProviderSpec {
    pub name: ProviderId,
    pub wire: WireProtocol,
    pub surface: ApiSurface,
    pub endpoint: String,
    pub api_key_secret: SecretName,
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub provider: ProviderId,
    pub id: ModelId,
    pub aliases: Vec<ModelId>,
    pub upstream_model: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderCatalogService;

impl ProviderCatalogService {
    pub fn upsert_provider(registry: &mut ProviderRegistry, spec: ProviderSpec) {
        let models = registry
            .find_provider(spec.name.as_str())
            .map(|p| p.models.clone())
            .unwrap_or_default();
        registry
            .providers
            .retain(|p| p.name.as_str() != spec.name.as_str());
        registry.providers.push(ProviderEntry {
            name: spec.name,
            wire: spec.wire,
            surface: spec.surface,
            endpoint: spec.endpoint,
            api_key_secret: spec.api_key_secret,
            extra_headers: spec.extra_headers,
            models,
        });
    }

    pub fn remove_provider(registry: &mut ProviderRegistry, name: &ProviderId) -> ConfigResult<()> {
        let before = registry.providers.len();
        registry
            .providers
            .retain(|p| p.name.as_str() != name.as_str());
        if registry.providers.len() == before {
            return Err(ConfigError::ProviderNotFound {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    pub fn upsert_model(registry: &mut ProviderRegistry, spec: ModelSpec) -> ConfigResult<()> {
        let provider = registry
            .providers
            .iter_mut()
            .find(|p| p.name.as_str() == spec.provider.as_str())
            .ok_or_else(|| ConfigError::ProviderNotFound {
                name: spec.provider.to_string(),
            })?;
        provider
            .models
            .retain(|m| m.id.as_str() != spec.id.as_str());
        provider.models.push(ProviderModel {
            id: spec.id,
            aliases: spec.aliases,
            upstream_model: spec.upstream_model,
            pricing: ModelPricing::default(),
            capabilities: ModelCapabilities::default(),
            limits: ModelLimits::default(),
        });
        Ok(())
    }

    pub fn remove_model(
        registry: &mut ProviderRegistry,
        provider_name: &ProviderId,
        id: &ModelId,
    ) -> ConfigResult<()> {
        let provider = registry
            .providers
            .iter_mut()
            .find(|p| p.name.as_str() == provider_name.as_str())
            .ok_or_else(|| ConfigError::ProviderNotFound {
                name: provider_name.to_string(),
            })?;
        let before = provider.models.len();
        provider.models.retain(|m| m.id.as_str() != id.as_str());
        if provider.models.len() == before {
            return Err(ConfigError::ModelNotFound {
                id: id.to_string(),
                provider: provider_name.to_string(),
            });
        }
        Ok(())
    }
}
