//! [`EndpointBinding`]: the catalog provider, endpoint and credential one
//! upstream is built from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::catalog::{
    EndpointRef, ResolvedCatalog, ResolvedCredential, ResolvedEndpoint, ResolvedHop,
    ResolvedProvider,
};

use super::error::UpstreamTargetError;

/// The three catalog objects one upstream is built from.
#[derive(Debug, Clone, Copy)]
pub struct EndpointBinding<'a> {
    pub provider: &'a ResolvedProvider,
    pub endpoint: &'a ResolvedEndpoint,
    pub credential: &'a ResolvedCredential,
}

impl<'a> EndpointBinding<'a> {
    pub fn of_hop(
        catalog: &'a ResolvedCatalog,
        hop: &'a ResolvedHop,
    ) -> Result<Self, UpstreamTargetError> {
        Self::of_endpoint(catalog, &hop.endpoint_ref())
    }

    pub fn of_endpoint(
        catalog: &'a ResolvedCatalog,
        endpoint: &EndpointRef,
    ) -> Result<Self, UpstreamTargetError> {
        let unknown = || UpstreamTargetError::UnknownEndpoint {
            endpoint: endpoint.to_string(),
        };
        let resolved = catalog.endpoint(endpoint).ok_or_else(unknown)?;
        let provider = catalog
            .provider(endpoint.provider.as_str())
            .ok_or_else(unknown)?;
        let credential = catalog
            .credential(provider.credential.as_str())
            .ok_or_else(unknown)?;
        Ok(Self {
            provider,
            endpoint: resolved,
            credential,
        })
    }
}
