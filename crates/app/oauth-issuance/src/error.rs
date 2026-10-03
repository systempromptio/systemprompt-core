//! Typed failure of token issuance.
//!
//! [`IssuanceError`] partitions failures by RFC 6749 §5.2 error code; the HTTP
//! layer maps each variant onto its wire error body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::errors::GlobalConfigError;
use systemprompt_oauth::OauthError;
use systemprompt_oauth::services::validation::id_jag::IdJagError;
use systemprompt_traits::BoxedSource;

pub type IssuanceResult<T> = Result<T, IssuanceError>;

/// Failure of a token issuance, one variant per RFC 6749 error family.
#[derive(Debug, thiserror::Error)]
pub enum IssuanceError {
    #[error("Invalid request: {field} {message}")]
    InvalidRequest { field: String, message: String },

    #[error("Invalid request: {field} {reason}")]
    MalformedField {
        field: &'static str,
        reason: &'static str,
        #[source]
        source: BoxedSource,
    },

    #[error("Unsupported grant type: {grant_type}")]
    UnsupportedGrantType { grant_type: String },

    #[error("Invalid client credentials")]
    InvalidClient,

    #[error("Invalid authorization code: {reason}")]
    InvalidGrant { reason: String },

    #[error("Invalid grant: {reason}")]
    RejectedGrant {
        reason: &'static str,
        #[source]
        source: BoxedSource,
    },

    #[error("Invalid refresh token: {reason}")]
    InvalidRefreshToken { reason: String },

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid client secret")]
    InvalidClientSecret,

    #[error("Authorization code expired")]
    ExpiredCode,

    #[error("Server error: {context}")]
    ServerError {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },

    #[error("Invalid target resource: {message}")]
    InvalidTarget { message: String },

    #[error("Invalid scope: {message}")]
    InvalidScope { message: String },

    #[error("Invalid grant: {0}")]
    IdJagRejected(#[source] IdJagError),

    #[error("Invalid target resource: {0}")]
    BoundResource(#[source] IdJagError),

    #[error(transparent)]
    Oauth(#[from] OauthError),
}

impl IssuanceError {
    pub fn server(context: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::ServerError {
            context,
            source: source.into(),
        }
    }

    pub fn malformed(
        field: &'static str,
        reason: &'static str,
        source: impl Into<BoxedSource>,
    ) -> Self {
        Self::MalformedField {
            field,
            reason,
            source: source.into(),
        }
    }

    pub fn rejected_grant(reason: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::RejectedGrant {
            reason,
            source: source.into(),
        }
    }
}

impl From<GlobalConfigError> for IssuanceError {
    fn from(error: GlobalConfigError) -> Self {
        Self::server("Configuration unavailable", error)
    }
}
