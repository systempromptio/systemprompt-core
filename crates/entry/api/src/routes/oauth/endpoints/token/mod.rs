//! OAuth 2.0 token endpoint.
//!
//! Hosts the `/token` handler and the request/response types
//! ([`TokenRequest`], [`TokenResponse`]) it binds. Per-grant token minting
//! lives in [`generation`]; [`TokenError`] partitions failures by RFC 6749
//! error code and maps onto the HTTP error surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod generation;
pub mod handler;
pub mod validation;

pub use handler::handle_token;

use serde::{Deserialize, Serialize};
use systemprompt_models::errors::GlobalConfigError;
use systemprompt_oauth::OauthError;
use systemprompt_oauth::services::validation::id_jag::IdJagError;
use systemprompt_traits::BoxedSource;

use crate::routes::oauth::{OAuthHttpError, internal};

pub type TokenResult<T> = Result<T, TokenError>;

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub code_verifier: Option<String>,
    pub resource: Option<String>,
    pub plugin_id: Option<String>,
    pub audience: Option<String>,
    pub subject_token: Option<String>,
    pub subject_token_type: Option<String>,
    pub actor_token: Option<String>,
    pub actor_token_type: Option<String>,
    pub requested_token_type: Option<String>,
    pub assertion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_token_type: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
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

impl TokenError {
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

impl From<GlobalConfigError> for TokenError {
    fn from(error: GlobalConfigError) -> Self {
        Self::server("Configuration unavailable", error)
    }
}

impl From<TokenError> for OAuthHttpError {
    fn from(error: TokenError) -> Self {
        match error {
            TokenError::InvalidRequest { field, message } => {
                Self::invalid_request(format!("{field}: {message}"))
            },
            TokenError::MalformedField {
                field,
                reason,
                source,
            } => internal::rejected(Self::invalid_request(format!("{field}: {reason}")), source),
            TokenError::UnsupportedGrantType { grant_type } => {
                Self::unsupported_grant_type(format!("Grant type '{grant_type}' is not supported"))
            },
            TokenError::InvalidClient => Self::invalid_client("Client authentication failed"),
            TokenError::InvalidGrant { reason } => Self::invalid_grant(reason),
            TokenError::RejectedGrant { reason, source } => {
                internal::rejected(Self::invalid_grant(reason), source)
            },
            TokenError::InvalidRefreshToken { reason } => {
                Self::invalid_grant(format!("Refresh token invalid: {reason}"))
            },
            TokenError::InvalidCredentials => Self::invalid_grant("Invalid credentials"),
            TokenError::InvalidClientSecret => Self::invalid_client("Invalid client secret"),
            TokenError::ExpiredCode => Self::invalid_grant("Authorization code expired"),
            TokenError::ServerError { context, source } => internal::server_error(context, source),
            TokenError::InvalidTarget { message } => Self::invalid_target(message),
            TokenError::InvalidScope { message } => Self::invalid_scope(message),
            TokenError::IdJagRejected(error) => Self::invalid_grant(error.to_string()),
            TokenError::BoundResource(error) => Self::invalid_target(error.to_string()),
            TokenError::Oauth(error) => Self::from(error),
        }
    }
}
