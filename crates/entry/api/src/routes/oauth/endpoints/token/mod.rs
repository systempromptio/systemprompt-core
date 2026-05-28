pub mod generation;
mod handler;
pub mod validation;

pub use handler::handle_token;

use serde::{Deserialize, Serialize};

use crate::routes::oauth::OAuthHttpError;

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
    // RFC 8693 §2.2.1 issued_token_type. Only set by the
    // urn:ietf:params:oauth:grant-type:token-exchange flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_token_type: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Invalid request: {field} {message}")]
    InvalidRequest { field: String, message: String },

    #[error("Unsupported grant type: {grant_type}")]
    UnsupportedGrantType { grant_type: String },

    #[error("Invalid client credentials")]
    InvalidClient,

    #[error("Invalid authorization code: {reason}")]
    InvalidGrant { reason: String },

    #[error("Invalid refresh token: {reason}")]
    InvalidRefreshToken { reason: String },

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid client secret")]
    InvalidClientSecret,

    #[error("Authorization code expired")]
    ExpiredCode,

    #[error("Server error: {message}")]
    ServerError { message: String },

    #[error("Invalid target resource: {message}")]
    InvalidTarget { message: String },

    #[error("Invalid scope: {message}")]
    InvalidScope { message: String },
}

impl From<TokenError> for OAuthHttpError {
    fn from(error: TokenError) -> Self {
        match error {
            TokenError::InvalidRequest { field, message } => {
                Self::invalid_request(format!("{field}: {message}"))
            },
            TokenError::UnsupportedGrantType { grant_type } => {
                Self::unsupported_grant_type(format!("Grant type '{grant_type}' is not supported"))
            },
            TokenError::InvalidClient => Self::invalid_client("Client authentication failed"),
            TokenError::InvalidGrant { reason } => Self::invalid_grant(reason),
            TokenError::InvalidRefreshToken { reason } => {
                Self::invalid_grant(format!("Refresh token invalid: {reason}"))
            },
            TokenError::InvalidCredentials => Self::invalid_grant("Invalid credentials"),
            TokenError::InvalidClientSecret => Self::invalid_client("Invalid client secret"),
            TokenError::ExpiredCode => Self::invalid_grant("Authorization code expired"),
            TokenError::ServerError { message } => Self::server_error(message),
            TokenError::InvalidTarget { message } => Self::invalid_target(message),
            TokenError::InvalidScope { message } => Self::invalid_scope(message),
        }
    }
}
