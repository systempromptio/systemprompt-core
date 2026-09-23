//! [`UpstreamTarget`]: a catalog provider bound to its credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::ProviderId;
use systemprompt_models::services::{Hosting, ProviderEntry, WireProtocol};
use systemprompt_models::wire::anthropic::AnthropicBeta;
use systemprompt_models::wire::upstream::UpstreamDialect;
use systemprompt_security::credential::{CredentialKind, ProviderCredential, fill_endpoint};

use super::call::UpstreamCall;
use super::error::UpstreamTargetError;

/// A provider resolved once and reused for every request to it.
///
/// Holds the parsed credential rather than an auth header: an OAuth token
/// expires within the hour, so the header is minted per call from the shared
/// token cache, never frozen at construction.
#[derive(Debug, Clone)]
pub struct UpstreamTarget {
    provider: ProviderId,
    wire: WireProtocol,
    hosting: Hosting,
    endpoint: String,
    credential: ProviderCredential,
    cache_key: String,
    extra_headers: Vec<(String, String)>,
    accepted_betas: Option<BTreeSet<AnthropicBeta>>,
}

impl UpstreamTarget {
    pub fn resolve(entry: &ProviderEntry, secret: &str) -> Result<Self, UpstreamTargetError> {
        let secret_name = entry.api_key_secret.as_str();
        let credential = ProviderCredential::parse(secret).map_err(|source| {
            UpstreamTargetError::MalformedCredential {
                secret: secret_name.to_owned(),
                source,
            }
        })?;
        if entry.hosting() == Hosting::Vertex
            && entry.wire != WireProtocol::Gemini
            && credential.kind() == CredentialKind::ApiKey
        {
            return Err(UpstreamTargetError::ApiKeyOnVertex {
                provider: entry.name.as_str().to_owned(),
                secret: secret_name.to_owned(),
            });
        }
        let endpoint = fill_endpoint(&entry.endpoint, &credential.scope()).map_err(|source| {
            UpstreamTargetError::Endpoint {
                provider: entry.name.as_str().to_owned(),
                source,
            }
        })?;
        let mut extra_headers: Vec<(String, String)> = entry
            .extra_headers
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        extra_headers.sort();
        Ok(Self {
            provider: entry.name.clone(),
            wire: entry.wire,
            hosting: entry.hosting(),
            endpoint,
            credential,
            cache_key: secret_name.to_owned(),
            extra_headers,
            accepted_betas: entry.accepted_betas.clone(),
        })
    }

    pub fn from_secrets(entry: &ProviderEntry) -> Result<Self, UpstreamTargetError> {
        let secrets = systemprompt_config::SecretsBootstrap::get()
            .map_err(|e| UpstreamTargetError::SecretsUnavailable(e.to_string()))?;
        let secret_name = entry.api_key_secret.as_str();
        let secret =
            secrets
                .get(secret_name)
                .ok_or_else(|| UpstreamTargetError::MissingSecret {
                    provider: entry.name.as_str().to_owned(),
                    secret: secret_name.to_owned(),
                })?;
        Self::resolve(entry, secret)
    }

    pub async fn call(&self) -> Result<UpstreamCall, UpstreamTargetError> {
        let auth = self
            .credential
            .bearer(&self.cache_key)
            .await
            .map_err(|source| UpstreamTargetError::Mint {
                secret: self.cache_key.clone(),
                source,
            })?;
        Ok(UpstreamCall::new(
            self.hosting,
            self.endpoint.clone(),
            auth,
            self.extra_headers.clone(),
        )
        .with_accepted_betas(self.accepted_betas.clone()))
    }

    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    #[must_use]
    pub const fn wire(&self) -> WireProtocol {
        self.wire
    }

    #[must_use]
    pub const fn hosting(&self) -> Hosting {
        self.hosting
    }

    #[must_use]
    pub const fn dialect(&self) -> UpstreamDialect {
        UpstreamDialect::new(self.wire, self.hosting)
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[must_use]
    pub const fn credential(&self) -> &ProviderCredential {
        &self.credential
    }
}
