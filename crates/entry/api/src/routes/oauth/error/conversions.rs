//! `From` impls mapping domain errors onto [`OAuthHttpError`], keeping the
//! variant-to-RFC-code mapping in one place so handlers use `?`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_config::SecretsBootstrapError;
use systemprompt_models::errors::ConfigError;
use systemprompt_oauth::{OauthError, OauthErrorKind};
use systemprompt_traits::auth::AuthProviderError;

use super::OAuthHttpError;

impl From<ConfigError> for OAuthHttpError {
    fn from(err: ConfigError) -> Self {
        Self::server_error("Configuration unavailable").with_source(err)
    }
}

impl From<OauthError> for OAuthHttpError {
    fn from(err: OauthError) -> Self {
        match err.kind() {
            OauthErrorKind::InvalidClient => Self::invalid_client("Client authentication failed"),
            OauthErrorKind::InvalidClientMetadata => Self::invalid_client_metadata(err.to_string()),
            OauthErrorKind::InvalidGrant => Self::invalid_grant(err.to_string()),
            OauthErrorKind::InvalidToken => Self::invalid_token(err.to_string()),
            OauthErrorKind::InvalidRequest => Self::invalid_request(err.to_string()),
            OauthErrorKind::AccessDenied => Self::access_denied(err.to_string()),
            OauthErrorKind::UsernameUnavailable => Self::username_unavailable(
                "Username is already taken. Please choose a different username.",
            ),
            OauthErrorKind::EmailExists => {
                Self::email_exists("An account with this email already exists.")
            },
            OauthErrorKind::NotFound => Self::not_found(err.to_string()),
            OauthErrorKind::ExpiredChallenge => Self::expired_challenge(
                "The challenge has expired. Please start the ceremony again.",
            ),
            OauthErrorKind::InvalidCredential => Self::invalid_credential(
                "WebAuthn verification failed. Please ensure your authenticator and browser are \
                 compatible.",
            ),
            OauthErrorKind::AuthenticationFailed => Self::authentication_failed(
                "Authentication failed. Check the email address or register a passkey.",
            ),
            OauthErrorKind::ServerError => {
                Self::server_error("Authorization operation failed").with_source(err)
            },
        }
    }
}

impl From<AuthProviderError> for OAuthHttpError {
    fn from(err: AuthProviderError) -> Self {
        match err {
            e @ (AuthProviderError::InvalidCredentials | AuthProviderError::InvalidToken) => {
                Self::invalid_client(e.to_string())
            },
            e @ AuthProviderError::UserNotFound => Self::not_found(e.to_string()),
            e @ AuthProviderError::TokenExpired => Self::invalid_grant(e.to_string()),
            e @ AuthProviderError::InsufficientPermissions => Self::access_denied(e.to_string()),
            other => Self::server_error("Authentication provider failed").with_source(other),
        }
    }
}

impl From<SecretsBootstrapError> for OAuthHttpError {
    fn from(err: SecretsBootstrapError) -> Self {
        Self::server_error("Secrets unavailable").with_source(err)
    }
}

impl From<sqlx::Error> for OAuthHttpError {
    fn from(err: sqlx::Error) -> Self {
        Self::server_error("Database operation failed").with_source(err)
    }
}

impl From<anyhow::Error> for OAuthHttpError {
    fn from(err: anyhow::Error) -> Self {
        Self::server_error("Authorization operation failed").with_source(err)
    }
}
