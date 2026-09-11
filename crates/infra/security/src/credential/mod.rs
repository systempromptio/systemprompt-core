//! One model for every upstream credential this instance holds.
//!
//! A secret is parsed once into a [`ProviderCredential`]; everything
//! downstream asks the credential for its scope and its auth header and never
//! inspects the secret again. That single rule is what makes the next
//! credential type — a workload identity, an OIDC client, a scoped key with a
//! tenant id — a new variant here rather than another branch in the gateway,
//! the loader and the bridge.
//!
//! The kinds are told apart by the secret's own content rather than by a
//! catalog flag. A Google service-account key is a JSON document that names
//! itself in a `type` field (`"service_account"`), which is an explicit,
//! Google-defined self-description rather than a guess about shape; anything
//! that is not such a document is an API key. So the provider catalog carries
//! no credential-type column, and an operator who pastes a service account
//! gets the right behaviour without having to know a flag exists.
//! [`ProviderCredential::parse`] is the only place that decision is made.
//!
//! A minted token is cached under the secret *name*, not its value, so two
//! providers sharing a secret share one minted token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cache;
mod error;
pub(crate) mod http;
mod scope;

use std::fmt;

pub use error::CredentialError;
pub use scope::{
    AuthHeader, AuthScheme, CredentialScope, PROJECT_PLACEHOLDER, REGION_PLACEHOLDER, fill_endpoint,
};

use crate::google::{SERVICE_ACCOUNT_TYPE, ServiceAccountKey, access_token};

/// An API key, held in a type that will not print itself. `expose` hands out
/// the key itself; every call site is one that is about to send it.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKeySecret(String);

impl ApiKeySecret {
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKeySecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKeySecret(<redacted>)")
    }
}

/// What kind of credential a secret turned out to hold.
///
/// Stable strings: they key the token cache and name the kind in operator
/// output, so they are not derived from the variant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    ApiKey,
    GoogleServiceAccount,
}

impl CredentialKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::GoogleServiceAccount => "google_service_account",
        }
    }
}

/// A credential an upstream provider will accept, parsed from its secret.
///
/// An API key is the credential and is sent verbatim; a Google service-account
/// key is exchanged for a short-lived OAuth bearer token. `scope` is the
/// coordinates the credential supplies to the endpoint it authenticates, and
/// `fill_endpoint` resolves a catalog endpoint template against them.
#[derive(Debug, Clone)]
pub enum ProviderCredential {
    ApiKey(ApiKeySecret),
    GoogleServiceAccount(Box<ServiceAccountKey>),
}

impl ProviderCredential {
    pub fn parse(secret: &str) -> Result<Self, CredentialError> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(secret) else {
            return Ok(Self::ApiKey(ApiKeySecret(secret.to_owned())));
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some(SERVICE_ACCOUNT_TYPE) {
            return Ok(Self::ApiKey(ApiKeySecret(secret.to_owned())));
        }
        serde_json::from_value::<ServiceAccountKey>(value)
            .map(|key| Self::GoogleServiceAccount(Box::new(key)))
            .map_err(|e| CredentialError::Malformed(e.to_string()))
    }

    #[must_use]
    pub const fn kind(&self) -> CredentialKind {
        match self {
            Self::ApiKey(_) => CredentialKind::ApiKey,
            Self::GoogleServiceAccount(_) => CredentialKind::GoogleServiceAccount,
        }
    }

    #[must_use]
    pub fn scope(&self) -> CredentialScope {
        match self {
            // Why: an API key is a bare string. It names no project, no region
            // and no principal, and inventing any of them would send a request
            // somewhere the operator never chose.
            Self::ApiKey(_) => CredentialScope::empty(),
            Self::GoogleServiceAccount(key) => CredentialScope {
                project: Some(key.project_id.clone()),
                region: None,
                principal: Some(key.client_email.clone()),
            },
        }
    }

    pub async fn bearer(&self, cache_key: &str) -> Result<AuthHeader, CredentialError> {
        match self {
            Self::ApiKey(key) => Ok(AuthHeader {
                scheme: AuthScheme::ApiKey,
                value: key.expose().to_owned(),
            }),
            Self::GoogleServiceAccount(key) => {
                let key_id = format!("{}:{cache_key}", self.kind().as_str());
                Ok(AuthHeader {
                    scheme: AuthScheme::Bearer,
                    value: access_token(&key_id, key).await?,
                })
            },
        }
    }

    pub fn fill_endpoint(&self, template: &str) -> Result<String, CredentialError> {
        fill_endpoint(template, &self.scope())
    }
}
