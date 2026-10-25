//! Failures resolving a catalog endpoint into a sendable upstream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_security::credential::CredentialError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpstreamTargetError {
    #[error("endpoint '{endpoint}' is not in the resolved catalog")]
    UnknownEndpoint { endpoint: String },

    #[error("credential '{credential}' secret '{secret}' is not configured")]
    MissingSecret { credential: String, secret: String },

    #[error(
        "credential '{credential}' is personal: each member stores their own key, so no \
         deployment secret serves it"
    )]
    PersonalCredential { credential: String },

    #[error(
        "credential '{credential}' declares a Google service account but is malformed: {source}"
    )]
    MalformedCredential {
        credential: String,
        #[source]
        source: CredentialError,
    },

    #[error(
        "endpoint '{endpoint}' is hosted on Vertex AI, which accepts only a Google \
         service-account key for this wire; credential '{credential}' holds an API key"
    )]
    ApiKeyOnVertex {
        endpoint: String,
        credential: String,
    },

    #[error("endpoint '{endpoint}' URL cannot be resolved: {source}")]
    Endpoint {
        endpoint: String,
        #[source]
        source: CredentialError,
    },

    #[error("could not mint a Google access token for credential '{credential}': {source}")]
    Mint {
        credential: String,
        #[source]
        source: CredentialError,
    },
}
