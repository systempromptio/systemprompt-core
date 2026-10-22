//! Provider-registry mutations behind the `admin config catalog` surface.
//!
//! [`ProviderCatalogService`] edits a typed [`ProviderRegistry`] — the
//! `providers:` list of `services/ai/providers.yaml`: declaring or removing
//! upstream providers and the models each provider serves. Upserts replace an
//! entry in place; a provider upsert preserves the existing model catalog so
//! connectivity can be re-declared without re-listing models, and every field
//! the spec does not carry (credential source, `credential_from`, user agent,
//! forwarded and extra headers, accepted betas) survives the upsert. Callers
//! revalidate and persist the services file after a successful mutation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_manifest::services::ai::{ModelCapabilities, ModelGovernance, ModelPricing};
use systemprompt_manifest::services::{
    ProviderCredentialSource, ProviderEntry, ProviderModel, ProviderRegistry,
};
use systemprompt_models::providers::ApiSurface;
use systemprompt_wire::{ModelLimits, WireProtocol};

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
        let existing = registry
            .providers
            .iter()
            .position(|p| p.name.as_str() == spec.name.as_str())
            .map(|idx| registry.providers.remove(idx));
        let mut entry = existing.unwrap_or_else(|| ProviderEntry {
            name: spec.name.clone(),
            display_name: None,
            description: None,
            wire: spec.wire,
            surface: spec.surface,
            endpoint: String::new(),
            api_key_secret: None,
            credential: ProviderCredentialSource::Platform,
            credential_from: None,
            user_agent: None,
            forward_headers: Vec::new(),
            extra_headers: HashMap::new(),
            accepted_betas: None,
            models: Vec::new(),
            governance: ModelGovernance::default(),
        });
        entry.wire = spec.wire;
        entry.surface = spec.surface;
        entry.endpoint = spec.endpoint;
        entry.api_key_secret = Some(spec.api_key_secret);
        entry.extra_headers.extend(spec.extra_headers);
        registry.providers.push(entry);
    }

    pub fn remove_provider(registry: &mut ProviderRegistry, name: &ProviderId) -> ConfigResult<()> {
        let before = registry.providers.len();
        registry
            .providers
            .retain(|p| p.name.as_str() != name.as_str());
        if registry.providers.len() == before {
            return Err(ConfigError::ProviderNotFound { name: name.clone() });
        }
        Ok(())
    }

    pub fn upsert_model(registry: &mut ProviderRegistry, spec: ModelSpec) -> ConfigResult<()> {
        let provider = registry
            .providers
            .iter_mut()
            .find(|p| p.name.as_str() == spec.provider.as_str())
            .ok_or_else(|| ConfigError::ProviderNotFound {
                name: spec.provider.clone(),
            })?;
        provider
            .models
            .retain(|m| m.id.as_str() != spec.id.as_str());
        provider.models.push(ProviderModel {
            id: spec.id,
            aliases: spec.aliases,
            hidden: false,
            upstream_model: spec.upstream_model,
            pricing: ModelPricing::default(),
            capabilities: ModelCapabilities::default(),
            limits: ModelLimits::default(),
            governance: None,
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
                name: provider_name.clone(),
            })?;
        let before = provider.models.len();
        provider.models.retain(|m| m.id.as_str() != id.as_str());
        if provider.models.len() == before {
            return Err(ConfigError::ModelNotFound {
                id: id.clone(),
                provider: provider_name.clone(),
            });
        }
        Ok(())
    }
}
