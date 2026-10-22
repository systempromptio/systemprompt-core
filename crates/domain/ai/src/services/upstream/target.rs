//! [`UpstreamTarget`]: one catalog endpoint bound to its credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::{CredentialId, ProviderId, UserId};
use systemprompt_manifest::services::catalog::{
    EndpointRef, ResolvedCatalog, ResolvedCredential, ResolvedEndpoint, ResolvedHop,
    ResolvedProvider,
};
use systemprompt_security::credential::{
    CredentialKind, ProviderCredential, TokenCacheKey, fill_endpoint,
};
use systemprompt_wire::anthropic::AnthropicBeta;
use systemprompt_wire::upstream::UpstreamDialect;
use systemprompt_wire::{Hosting, WireProtocol};

use super::call::UpstreamCall;
use super::error::UpstreamTargetError;

/// An endpoint resolved once and reused for every request to it.
///
/// The wire, URL, hosting and accepted betas are the endpoint's; the headers,
/// user agent and forwarded-header allowlist are its provider's; the secret is
/// its provider's credential. It holds the parsed credential rather than an
/// auth header: an OAuth token expires within the hour, so the header is
/// minted per call from the credential-keyed token cache.
#[derive(Debug, Clone)]
pub struct UpstreamTarget {
    endpoint_ref: EndpointRef,
    credential_id: CredentialId,
    wire: WireProtocol,
    hosting: Hosting,
    endpoint: String,
    credential: ProviderCredential,
    cache_key: TokenCacheKey,
    extra_headers: Vec<(String, String)>,
    accepted_betas: Option<BTreeSet<AnthropicBeta>>,
    user_agent: Option<String>,
    forward_headers: Vec<String>,
}

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

impl UpstreamTarget {
    pub fn resolve(
        catalog: &ResolvedCatalog,
        hop: &ResolvedHop,
        secret: &str,
    ) -> Result<Self, UpstreamTargetError> {
        let binding = EndpointBinding::of_hop(catalog, hop)?;
        let key = TokenCacheKey::platform(&binding.credential.id);
        Self::bind(binding, secret, key)
    }

    pub fn resolve_personal(
        binding: EndpointBinding<'_>,
        key: &str,
        user_id: &UserId,
    ) -> Result<Self, UpstreamTargetError> {
        let cache_key = TokenCacheKey::personal(&binding.credential.id, user_id);
        Self::bind(binding, key, cache_key)
    }

    pub fn bind(
        binding: EndpointBinding<'_>,
        secret: &str,
        cache_key: TokenCacheKey,
    ) -> Result<Self, UpstreamTargetError> {
        let EndpointBinding {
            provider,
            endpoint,
            credential: catalog_credential,
        } = binding;
        let endpoint_ref = endpoint.endpoint_ref();
        let credential = ProviderCredential::parse(secret).map_err(|source| {
            UpstreamTargetError::MalformedCredential {
                credential: catalog_credential.id.as_str().to_owned(),
                source,
            }
        })?;
        if endpoint.hosting == Hosting::Vertex
            && endpoint.wire != WireProtocol::Gemini
            && credential.kind() == CredentialKind::ApiKey
        {
            return Err(UpstreamTargetError::ApiKeyOnVertex {
                endpoint: endpoint_ref.to_string(),
                credential: catalog_credential.id.as_str().to_owned(),
            });
        }
        let url = fill_endpoint(&endpoint.url, &credential.scope()).map_err(|source| {
            UpstreamTargetError::Endpoint {
                endpoint: endpoint_ref.to_string(),
                source,
            }
        })?;
        let mut extra_headers: Vec<(String, String)> = provider
            .extra_headers
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        extra_headers.sort();
        Ok(Self {
            endpoint_ref,
            credential_id: catalog_credential.id.clone(),
            wire: endpoint.wire,
            hosting: endpoint.hosting,
            endpoint: url,
            credential,
            cache_key,
            extra_headers,
            accepted_betas: endpoint.betas.clone(),
            user_agent: provider.user_agent.clone(),
            forward_headers: provider
                .forward_headers
                .iter()
                .map(|name| name.to_ascii_lowercase())
                .collect(),
        })
    }

    pub fn from_secrets(binding: EndpointBinding<'_>) -> Result<Self, UpstreamTargetError> {
        let credential = binding.credential;
        let Some(secret_name) = credential.platform_secret() else {
            return Err(UpstreamTargetError::PersonalCredential {
                credential: credential.id.as_str().to_owned(),
            });
        };
        let secrets = systemprompt_config::SecretsBootstrap::get()
            .map_err(|e| UpstreamTargetError::SecretsUnavailable(Box::new(e)))?;
        let secret = secrets.get(secret_name.as_str()).ok_or_else(|| {
            UpstreamTargetError::MissingSecret {
                credential: credential.id.as_str().to_owned(),
                secret: secret_name.as_str().to_owned(),
            }
        })?;
        let key = TokenCacheKey::platform(&credential.id);
        Self::bind(binding, secret, key)
    }

    pub async fn call(&self) -> Result<UpstreamCall, UpstreamTargetError> {
        let auth = self
            .credential
            .bearer(&self.cache_key)
            .await
            .map_err(|source| UpstreamTargetError::Mint {
                credential: self.credential_id.as_str().to_owned(),
                source,
            })?;
        Ok(UpstreamCall::new(
            self.hosting,
            self.endpoint.clone(),
            auth,
            self.extra_headers.clone(),
        )
        .with_accepted_betas(self.accepted_betas.clone())
        .with_user_agent(self.user_agent.clone())
        .with_forward_headers(self.forward_headers.clone()))
    }

    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.endpoint_ref.provider
    }

    #[must_use]
    pub const fn endpoint_ref(&self) -> &EndpointRef {
        &self.endpoint_ref
    }

    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
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
    pub const fn accepted_betas(&self) -> Option<&BTreeSet<AnthropicBeta>> {
        self.accepted_betas.as_ref()
    }

    #[must_use]
    pub const fn cache_key(&self) -> &TokenCacheKey {
        &self.cache_key
    }

    #[must_use]
    pub const fn credential(&self) -> &ProviderCredential {
        &self.credential
    }
}
