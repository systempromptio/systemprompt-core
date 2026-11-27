//! [`FcmAccess`]: the access token Google's push service authorizes with.
//!
//! The declared secret holds a service-account key. The access token is
//! minted from it by the instance's Google credential exchange and reused
//! from that exchange's [`TokenCache`] until shortly before it expires; the
//! key is read from the secret store at each mint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::SecretName;
use systemprompt_security::credential::TokenCache;
use systemprompt_security::google::{ServiceAccountKey, access_token};
use systemprompt_traits::SecretStore;

use crate::channels::push::{ProviderBearer, ProviderCredential, PushError};

const MAX_PROJECT_ID_LEN: usize = 64;

/// The Google project a service account belongs to and its tokens address:
/// ASCII lowercase letters, digits and `-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FcmProjectId(String);

/// A string that is not a valid [`FcmProjectId`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid Google project id {value:?}")]
pub struct FcmProjectIdError {
    pub value: String,
}

impl FcmProjectId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FcmProjectIdError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= MAX_PROJECT_ID_LEN
            && value
                .bytes()
                .all(|b| b == b'-' || b.is_ascii_lowercase() || b.is_ascii_digit());
        if valid {
            Ok(Self(value))
        } else {
            Err(FcmProjectIdError { value })
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Mints and caches the access token of one service account.
pub struct FcmAccess {
    secrets: Arc<dyn SecretStore>,
    secret: SecretName,
    tokens: TokenCache,
}

impl FcmAccess {
    #[must_use]
    pub fn new(secrets: Arc<dyn SecretStore>, secret: SecretName) -> Self {
        Self {
            secrets,
            secret,
            tokens: TokenCache::new(),
        }
    }

    fn key(&self) -> Result<ServiceAccountKey, PushError> {
        let stored = self.secrets.secret(self.secret.as_str()).ok_or_else(|| {
            PushError::CredentialMissing {
                secret: self.secret.clone(),
            }
        })?;
        serde_json::from_str(stored.expose()).map_err(|source| PushError::CredentialInvalid {
            secret: self.secret.clone(),
            source: Box::new(source),
        })
    }

    pub fn project(&self) -> Result<FcmProjectId, PushError> {
        FcmProjectId::try_new(self.key()?.project_id).map_err(|source| {
            PushError::CredentialInvalid {
                secret: self.secret.clone(),
                source: Box::new(source),
            }
        })
    }
}

impl fmt::Debug for FcmAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FcmAccess")
            .field("secret", &self.secret)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ProviderCredential for FcmAccess {
    async fn bearer(&self) -> Result<ProviderBearer, PushError> {
        let key = self.key()?;
        access_token(&self.tokens, self.secret.as_str(), &key)
            .await
            .map(ProviderBearer::new)
            .map_err(|source| PushError::Authorization(Box::new(source)))
    }
}
