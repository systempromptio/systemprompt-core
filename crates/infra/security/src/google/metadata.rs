//! Workload identity: the runtime's ambient Google service account.
//!
//! On GKE (with Workload Identity), Cloud Run and Compute Engine the metadata
//! server mints an access token for the service account the workload runs
//! as, so no key file exists anywhere. This is the one implementation of that
//! mint: Cloud Storage and the AI gateway both reach it through a
//! [`TokenCache`], and the token is cached under the identity itself
//! ([`MetadataIdentity::cache_key`]), not under a consumer or a credential,
//! because every consumer of one workload holds the same principal.
//!
//! The metadata host is link-local and refused by the guarded outbound
//! client, so the exchange goes through the cache's own bounded client. The
//! token URL is a field so a test can point it at a local server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

use crate::credential::cache::{TokenCache, clamp_ttl};
use crate::credential::{CredentialError, http};

pub const DEFAULT_METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

// Why: the metadata server refuses a token request without this header, which
// is its guard against a request forwarded from outside the workload.
const METADATA_FLAVOR: (&str, &str) = ("Metadata-Flavor", "Google");

/// Where the runtime's ambient Google identity mints its tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataIdentity {
    token_url: String,
}

impl Default for MetadataIdentity {
    fn default() -> Self {
        Self::ambient()
    }
}

impl MetadataIdentity {
    #[must_use]
    pub fn ambient() -> Self {
        Self::at(DEFAULT_METADATA_TOKEN_URL)
    }

    #[must_use]
    pub fn at(token_url: impl Into<String>) -> Self {
        Self {
            token_url: token_url.into(),
        }
    }

    #[must_use]
    pub fn token_url(&self) -> &str {
        &self.token_url
    }

    #[must_use]
    pub fn cache_key(&self) -> String {
        format!("google_workload_identity:{}", self.token_url)
    }
}

#[derive(Debug, Deserialize)]
struct MetadataToken {
    access_token: String,
    #[serde(default, deserialize_with = "crate::oauth_wire::expires_in")]
    expires_in: Option<u64>,
}

pub async fn metadata_access_token(
    tokens: &TokenCache,
    identity: &MetadataIdentity,
) -> Result<String, CredentialError> {
    tokens
        .token_for(&identity.cache_key(), || async {
            let body = http::get(
                tokens.http_client()?,
                identity.token_url(),
                &[METADATA_FLAVOR],
            )
            .await?;
            let token: MetadataToken =
                serde_json::from_str(&body).map_err(CredentialError::UnreadableBody)?;
            Ok((token.access_token, clamp_ttl(token.expires_in)))
        })
        .await
}
