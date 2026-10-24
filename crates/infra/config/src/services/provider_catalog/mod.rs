//! Catalog mutations behind the `admin config catalog` surface.
//!
//! [`ProviderCatalogService`] edits the four catalog registries of a
//! [`CatalogDocument`] — credentials, providers (with their endpoints),
//! models and deployments — read from and written to a `services/ai`
//! directory ([`read_catalog`], [`write_catalog`]). Every mutation is applied
//! to a copy, the copy is re-resolved, and the document changes only when the
//! result builds; a refused edit leaves it untouched. Upserts keep every
//! field their spec leaves unset (see [`specs`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod io;
mod specs;

use systemprompt_identifiers::{CredentialId, ModelId, ProviderId};
use systemprompt_manifest::services::catalog::{
    CatalogContext, CatalogDocument, EndpointConfig, EndpointRef, ModelEntry, ProviderConfig,
};

pub use io::{read_catalog, write_catalog};
pub use specs::{CredentialSpec, EndpointSpec, ModelSpec, ProviderSpec};

use crate::error::{ConfigError, ConfigResult};

#[derive(Debug, Clone, Copy)]
pub struct ProviderCatalogService;

impl ProviderCatalogService {
    fn apply(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        edit: impl FnOnce(&mut CatalogDocument) -> ConfigResult<()>,
    ) -> ConfigResult<()> {
        let mut next = document.clone();
        edit(&mut next)?;
        next.resolve(context).map_err(ConfigError::CatalogRefused)?;
        *document = next;
        Ok(())
    }

    pub fn upsert_credential(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        spec: CredentialSpec,
    ) -> ConfigResult<()> {
        Self::apply(document, context, |doc| {
            doc.credentials.credentials.insert(spec.id, spec.entry);
            Ok(())
        })
    }

    pub fn remove_credential(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        id: &CredentialId,
    ) -> ConfigResult<()> {
        Self::apply(document, context, |doc| {
            doc.credentials
                .credentials
                .remove(id)
                .map(|_| ())
                .ok_or_else(|| ConfigError::CredentialNotFound { id: id.clone() })
        })
    }

    pub fn upsert_provider(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        spec: ProviderSpec,
    ) -> ConfigResult<()> {
        Self::apply(document, context, |doc| {
            let provider = doc
                .providers
                .providers
                .entry(spec.id)
                .or_insert_with(|| ProviderConfig::new(spec.credential.as_str()));
            provider.credential = spec.credential;
            if let Some(display_name) = spec.display_name {
                provider.display_name = Some(display_name);
            }
            if let Some(user_agent) = spec.user_agent {
                provider.user_agent = Some(user_agent);
            }
            if let Some(enabled) = spec.enabled {
                provider.enabled = enabled;
            }
            provider.extra_headers.extend(spec.extra_headers);
            if let Some(endpoint) = spec.first_endpoint {
                let config = provider
                    .endpoints
                    .entry(endpoint.id)
                    .or_insert_with(|| EndpointConfig::new(endpoint.wire, endpoint.url.clone()));
                config.wire = endpoint.wire;
                config.url = endpoint.url;
            }
            Ok(())
        })
    }

    pub fn remove_provider(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        id: &ProviderId,
    ) -> ConfigResult<()> {
        Self::apply(document, context, |doc| {
            doc.providers
                .providers
                .remove(id)
                .map(|_| ())
                .ok_or_else(|| ConfigError::ProviderNotFound { name: id.clone() })
        })
    }

    pub fn upsert_endpoint(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        spec: EndpointSpec,
    ) -> ConfigResult<()> {
        Self::apply(document, context, |doc| {
            let provider = doc
                .providers
                .providers
                .get_mut(&spec.provider)
                .ok_or_else(|| ConfigError::ProviderNotFound {
                    name: spec.provider.clone(),
                })?;
            let endpoint = provider
                .endpoints
                .entry(spec.id)
                .or_insert_with(|| EndpointConfig::new(spec.wire, spec.url.clone()));
            endpoint.wire = spec.wire;
            endpoint.url = spec.url;
            Ok(())
        })
    }

    pub fn remove_endpoint(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        endpoint: &EndpointRef,
    ) -> ConfigResult<()> {
        Self::apply(document, context, |doc| {
            doc.providers
                .providers
                .get_mut(&endpoint.provider)
                .and_then(|p| p.endpoints.remove(&endpoint.endpoint))
                .map(|_| ())
                .ok_or_else(|| ConfigError::EndpointNotFound {
                    endpoint: endpoint.clone(),
                })
        })
    }

    pub fn upsert_model(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        spec: ModelSpec,
    ) -> ConfigResult<()> {
        if spec.deployments.is_empty() {
            return Err(ConfigError::ModelWithoutDeployments { id: spec.id });
        }
        Self::apply(document, context, |doc| {
            let clients = spec.clients.clone();
            let model = doc
                .models
                .models
                .entry(spec.id.clone())
                .or_insert_with(|| ModelEntry::new(clients.clone().unwrap_or_default()));
            if let Some(upstream_model) = spec.upstream_model {
                model.upstream_model = Some(upstream_model);
            }
            if let Some(display_name) = spec.display_name {
                model.display_name = Some(display_name);
            }
            if let Some(clients) = clients {
                model.clients = clients;
            }
            if let Some(pricing) = spec.pricing {
                model.pricing = pricing;
            }
            doc.deployments
                .deployments
                .insert(spec.id, spec.deployments);
            Ok(())
        })
    }

    pub fn remove_model(
        document: &mut CatalogDocument,
        context: &CatalogContext,
        id: &ModelId,
    ) -> ConfigResult<()> {
        Self::apply(document, context, |doc| {
            doc.deployments.deployments.remove(id);
            doc.models
                .models
                .remove(id)
                .map(|_| ())
                .ok_or_else(|| ConfigError::ModelNotFound { id: id.clone() })
        })
    }
}
