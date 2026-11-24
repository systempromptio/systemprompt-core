//! Request and client-metadata validation failures.
//!
//! Each variant's `Display` is the client-facing description an OAuth edge
//! answers with; [`super::OauthError`] prefixes it for logs and API errors.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ClientId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{reason}")]
    Rejected { reason: &'static str },

    #[error("Invalid scopes (roles): {}", scopes.join(", "))]
    UnknownScopes { scopes: Vec<String> },

    #[error("Scopes not available to self-registered clients: {}", scopes.join(", "))]
    ScopesNotSelfRegistrable { scopes: Vec<String> },

    #[error("Scopes not registered for this client: {}", scopes.join(", "))]
    ScopesNotRegisteredForClient { scopes: Vec<String> },

    #[error("Invalid redirect_uri: {uri}")]
    UnsafeRedirectUri { uri: String },

    #[error("redirect_uri {uri:?} must not contain a fragment")]
    RedirectUriFragment { uri: String },

    #[error("redirect_uri {uri:?} uses a refused scheme")]
    RedirectUriRefusedScheme { uri: String },

    #[error("redirect_uri {uri:?} must use https unless it targets the loopback interface")]
    RedirectUriInsecureHttp { uri: String },

    #[error(
        "redirect_uri {uri:?} uses a private-use scheme, which requires application_type \"native\""
    )]
    RedirectUriPrivateScheme { uri: String },

    #[error("{field} {value:?} must be an http(s) URL")]
    NonHttpUrl { field: String, value: String },

    #[error(
        "token_endpoint_auth_method must be one of none, client_secret_post, client_secret_basic, \
         got {method:?}"
    )]
    UnsupportedTokenAuthMethod { method: String },

    #[error("application_type must be \"web\" or \"native\", got {value:?}")]
    UnsupportedApplicationType { value: String },
}

#[derive(Debug, Error)]
pub enum ClientMetadataError {
    #[error("{reason}")]
    Rejected { reason: &'static str },

    #[error("CIMD metadata client_id mismatch: expected '{expected}', got '{got}'")]
    ClientIdMismatch { expected: ClientId, got: ClientId },

    #[error(
        "Invalid client_id format: '{client_id}'. Expected patterns:\n- https://* (CIMD \
         decentralized client)\n- sp_* (first-party systemprompt.io client)\n- client_* \
         (third-party registered client)\n- sys_* (internal system service)"
    )]
    UnrecognisedClientId { client_id: ClientId },

    #[error("redirect_uri '{uri}' not registered in CIMD metadata for {client_id}")]
    RedirectUriNotRegistered { uri: String, client_id: ClientId },
}
