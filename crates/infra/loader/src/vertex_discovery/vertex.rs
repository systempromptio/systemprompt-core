//! Vertex AI as a [`CatalogSource`].
//!
//! Everything Google-specific about discovery is here: the host suffix that
//! identifies a Vertex endpoint (every Vertex host ends in
//! `aiplatform.googleapis.com`; an endpoint that does not is some other
//! provider using a Google-shaped credential and is left alone — `vertex_host`
//! returns its origin, or `None`), the rule that only a Google service-account
//! key can list one, and the Model Garden listing call itself. The rate card
//! is held because it decides which publishers are worth asking about at all —
//! a provider the card prices nothing for has nothing to discover.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_models::services::{ProviderEntry, VertexRateCard};
use systemprompt_security::credential::{
    AuthHeader, CredentialKind, CredentialScope, ProviderCredential,
};

use super::client;
use super::source::{CatalogListing, CatalogSource, DiscoveryError};

const VERTEX_HOST_SUFFIX: &str = "aiplatform.googleapis.com";

#[derive(Debug)]
pub struct VertexCatalog {
    card: VertexRateCard,
}

impl VertexCatalog {
    #[must_use]
    pub const fn new(card: VertexRateCard) -> Self {
        Self { card }
    }

    fn publishers(&self, provider: &ProviderEntry) -> Vec<String> {
        self.card.publishers_for(provider.name.as_str())
    }
}

#[must_use]
pub fn vertex_host(endpoint: &str) -> Option<String> {
    let url = url::Url::parse(endpoint).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if host != VERTEX_HOST_SUFFIX && !host.ends_with(&format!("-{VERTEX_HOST_SUFFIX}")) {
        return None;
    }
    Some(format!("{}://{host}", url.scheme()))
}

#[async_trait]
impl CatalogSource for VertexCatalog {
    fn name(&self) -> &'static str {
        "vertex"
    }

    fn matches_provider(&self, provider: &ProviderEntry) -> bool {
        !self.publishers(provider).is_empty() && vertex_host(&provider.endpoint).is_some()
    }

    // Why: a provider keyed with something other than a service account is not
    // a discovery failure — it is an API key, and Vertex is simply not
    // reachable that way.
    fn applies(&self, provider: &ProviderEntry, credential: &ProviderCredential) -> bool {
        self.matches_provider(provider) && credential.kind() == CredentialKind::GoogleServiceAccount
    }

    async fn list(
        &self,
        http: &reqwest::Client,
        auth: &AuthHeader,
        provider: &ProviderEntry,
        _scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError> {
        let host = vertex_host(&provider.endpoint).ok_or_else(|| {
            DiscoveryError::Unusable(format!(
                "{}: endpoint '{}' is not a Vertex host",
                provider.name.as_str(),
                provider.endpoint
            ))
        })?;
        if !auth.is_bearer() {
            return Err(DiscoveryError::Unusable(format!(
                "{}: Model Garden requires a bearer token",
                provider.name.as_str()
            )));
        }

        let name = provider.name.as_str();
        let (models, failures) =
            client::list_all(http, &host, &auth.value, name, &self.publishers(provider)).await;

        Ok(CatalogListing {
            models: models
                .iter()
                .map(super::classify::PublisherModel::discovered)
                .collect(),
            failures,
        })
    }
}
