//! Vertex AI as a [`CatalogSource`].
//!
//! Everything Google-specific about discovery is here: the origin of a Vertex
//! endpoint (the host test is the catalog's one `is_vertex_host`; an endpoint
//! that fails it is some other provider using a Google-shaped credential and
//! is left alone — `vertex_host` returns its origin, or `None`), the rule
//! that only a Google service-account key can list one, and the Model Garden
//! listing call itself. The rate card
//! is held because it decides which publishers are worth asking about at all —
//! an endpoint whose wire the card prices nothing for has nothing to discover.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_manifest::services::VertexRateCard;
use systemprompt_security::credential::{
    AuthHeader, CredentialKind, CredentialScope, ProviderCredential,
};
use systemprompt_wire::hosting::is_vertex_host;

use super::client;
use super::source::{CatalogListing, CatalogSource, DiscoveryEndpoint, DiscoveryError};

#[derive(Debug)]
pub struct VertexCatalog {
    card: VertexRateCard,
}

impl VertexCatalog {
    #[must_use]
    pub const fn new(card: VertexRateCard) -> Self {
        Self { card }
    }

    fn publishers(&self, endpoint: &DiscoveryEndpoint) -> Vec<String> {
        self.card.publishers_for(endpoint.wire)
    }
}

#[must_use]
pub fn vertex_host(endpoint: &str) -> Option<String> {
    let url = url::Url::parse(endpoint).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !is_vertex_host(&host) {
        return None;
    }
    Some(format!("{}://{host}", url.scheme()))
}

#[async_trait]
impl CatalogSource for VertexCatalog {
    fn name(&self) -> &'static str {
        "vertex"
    }

    fn matches_endpoint(&self, endpoint: &DiscoveryEndpoint) -> bool {
        !self.publishers(endpoint).is_empty() && vertex_host(&endpoint.url).is_some()
    }

    fn applies(&self, endpoint: &DiscoveryEndpoint, credential: &ProviderCredential) -> bool {
        self.matches_endpoint(endpoint) && credential.kind() == CredentialKind::GoogleServiceAccount
    }

    async fn list(
        &self,
        http: &reqwest::Client,
        auth: &AuthHeader,
        endpoint: &DiscoveryEndpoint,
        _scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError> {
        let host = vertex_host(&endpoint.url).ok_or_else(|| DiscoveryError::NotVertexHost {
            endpoint: endpoint.url.clone(),
        })?;
        if !auth.is_bearer() {
            return Err(DiscoveryError::BearerRequired);
        }

        let label = endpoint.endpoint.to_string();
        let (models, failures) =
            client::list_all(http, &host, &auth.value, &label, &self.publishers(endpoint)).await;

        Ok(CatalogListing {
            models: models
                .iter()
                .map(super::classify::PublisherModel::discovered)
                .collect(),
            failures,
        })
    }
}
