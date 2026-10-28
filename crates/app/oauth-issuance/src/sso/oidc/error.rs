//! What the generic OIDC provider can fail on, with the typed cause kept.
//!
//! [`SsoError`] carries only a reason for an invalid assertion, so the cause
//! is logged once in the [`From`] impl and the browser-facing reason stays a
//! fixed string; transport and endpoint failures keep their cause as the
//! internal error's source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::net::OutboundUrlError;
use systemprompt_security::keys::JwksClientError;
use systemprompt_traits::sso::SsoError;

#[derive(Debug, thiserror::Error)]
pub enum OidcProviderError {
    #[error("discovery names issuer {discovered}, expected {expected}")]
    IssuerMismatch {
        expected: String,
        discovered: String,
    },
    #[error("OIDC endpoint `{field}` refused: {source}")]
    Endpoint {
        field: &'static str,
        #[source]
        source: OutboundUrlError,
    },
    #[error("OIDC discovery {url} failed: {source}")]
    Discovery {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("token endpoint request failed: {0}")]
    TokenEndpoint(#[source] reqwest::Error),
    #[error("token endpoint refused the code ({status}): {error}")]
    CodeRefused {
        status: reqwest::StatusCode,
        error: String,
    },
    #[error("token response carries no id_token")]
    NoIdToken,
    #[error("id_token header: {0}")]
    IdTokenHeader(#[source] jsonwebtoken::errors::Error),
    #[error("id_token carries a crit header")]
    CriticalHeader,
    #[error("id_token algorithm {0:?} is not allowed")]
    Algorithm(jsonwebtoken::Algorithm),
    #[error("id_token carries no kid")]
    NoKid,
    #[error("id_token signing key: {0}")]
    SigningKey(#[source] JwksClientError),
    #[error("id_token signing key components: {0}")]
    KeyComponents(#[source] jsonwebtoken::errors::Error),
    #[error("id_token rejected: {0}")]
    IdTokenRejected(#[source] jsonwebtoken::errors::Error),
    #[error("id_token nonce does not match the sign-in attempt")]
    Nonce,
    #[error("multi-audience id_token does not name this client in azp")]
    AuthorizedParty,
    #[error("id_token carries no sub")]
    NoSubject,
    #[error("identity provider answered `{0}`")]
    IdpError(String),
    #[error("callback carries no code")]
    NoCode,
    #[error("authorization response iss does not name the issuer")]
    ResponseIssuer,
}

impl OidcProviderError {
    const fn reason(&self) -> &'static str {
        match self {
            Self::IdTokenHeader(_) | Self::CriticalHeader => "id_token header",
            Self::Algorithm(_) => "id_token algorithm",
            Self::NoKid | Self::SigningKey(_) | Self::KeyComponents(_) => "id_token signing key",
            Self::IdTokenRejected(_) => "id_token signature or registered claims",
            Self::Nonce => "nonce",
            Self::AuthorizedParty => "azp",
            Self::NoSubject => "sub",
            Self::IdpError(_) => "identity provider error",
            Self::NoCode => "missing code",
            Self::ResponseIssuer => "iss",
            Self::CodeRefused { .. } => "code refused",
            Self::NoIdToken => "no id_token",
            Self::IssuerMismatch { .. }
            | Self::Endpoint { .. }
            | Self::Discovery { .. }
            | Self::TokenEndpoint(_) => "unavailable",
        }
    }
}

impl From<OidcProviderError> for SsoError {
    fn from(err: OidcProviderError) -> Self {
        match err {
            OidcProviderError::IssuerMismatch { .. }
            | OidcProviderError::Endpoint { .. }
            | OidcProviderError::Discovery { .. }
            | OidcProviderError::TokenEndpoint(_) => Self::internal(err),
            other => {
                tracing::warn!(error = %other, "OIDC sign-in rejected");
                Self::InvalidAssertion(other.reason().to_owned())
            },
        }
    }
}
