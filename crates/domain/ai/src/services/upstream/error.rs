//! Failures resolving a catalog provider into a sendable upstream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_security::credential::CredentialError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpstreamTargetError {
    #[error("secrets are not available: {0}")]
    SecretsUnavailable(String),

    #[error("provider '{provider}' secret '{secret}' is not configured")]
    MissingSecret { provider: String, secret: String },

    #[error("secret '{secret}' declares a Google service account but is malformed: {source}")]
    MalformedCredential {
        secret: String,
        #[source]
        source: CredentialError,
    },

    #[error(
        "provider '{provider}' is hosted on Vertex AI, which accepts only a Google service-account \
         key for this wire; secret '{secret}' holds an API key"
    )]
    ApiKeyOnVertex { provider: String, secret: String },

    #[error("provider '{provider}' endpoint cannot be resolved: {source}")]
    Endpoint {
        provider: String,
        #[source]
        source: CredentialError,
    },

    #[error("could not mint a Google access token from secret '{secret}': {source}")]
    Mint {
        secret: String,
        #[source]
        source: CredentialError,
    },
}
