//! `From` impls mapping domain errors onto [`OAuthHttpError`], keeping the
//! variant-to-RFC-code mapping in one place so handlers use `?`.

use systemprompt_config::SecretsBootstrapError;
use systemprompt_oauth::OauthError;
use systemprompt_traits::auth::AuthProviderError;

use super::{OAuthErrorCode, OAuthHttpError};

impl From<OauthError> for OAuthHttpError {
    fn from(err: OauthError) -> Self {
        match &err {
            OauthError::InvalidClient(_) | OauthError::ClientNotFound(_) => {
                Self::invalid_client(err.to_string())
            },
            OauthError::InvalidGrant(_)
            | OauthError::CodeNotFound(_)
            | OauthError::TokenNotFound(_)
            | OauthError::PkceMismatch(_)
            | OauthError::Expired(_) => Self::invalid_grant(err.to_string()),
            OauthError::Validation(_) => Self::invalid_request(err.to_string()),
            OauthError::Unauthorized(_) => Self::access_denied(err.to_string()),
            OauthError::UsernameTaken(_) => Self::username_unavailable(
                "Username is already taken. Please choose a different username.",
            ),
            OauthError::EmailRegistered(_) => {
                Self::email_exists("An account with this email already exists.")
            },
            OauthError::UserNotFound(_) => Self::not_found(err.to_string()),
            OauthError::RegistrationStateExpired => Self::expired_challenge(
                "Registration challenge has expired. Please start the registration process again.",
            ),
            OauthError::WebAuthnVerificationFailed(_) => Self::invalid_credential(
                "WebAuthn verification failed. Please ensure your authenticator and browser are \
                 compatible.",
            ),
            OauthError::WebAuthn(_)
            | OauthError::User(_)
            | OauthError::Session(_)
            | OauthError::TokenInvalid(_)
            | OauthError::TokenAlgMismatch { .. }
            | OauthError::TokenMissingKid
            | OauthError::TokenUnknownKid { .. }
            | OauthError::Provider(_)
            | OauthError::Repository(_)
            | OauthError::DatabaseRepository(_)
            | OauthError::Config(_)
            | OauthError::Crypto(_)
            | OauthError::Internal(_) => Self::server_error(err.to_string()),
        }
    }
}

impl From<AuthProviderError> for OAuthHttpError {
    fn from(err: AuthProviderError) -> Self {
        match &err {
            AuthProviderError::InvalidCredentials | AuthProviderError::InvalidToken => {
                Self::invalid_client(err.to_string())
            },
            AuthProviderError::UserNotFound => Self::not_found(err.to_string()),
            AuthProviderError::TokenExpired => Self::invalid_grant(err.to_string()),
            AuthProviderError::InsufficientPermissions => Self::access_denied(err.to_string()),
            _ => Self::server_error(err.to_string()),
        }
    }
}

impl From<SecretsBootstrapError> for OAuthHttpError {
    fn from(err: SecretsBootstrapError) -> Self {
        Self::server_error(err.to_string())
    }
}

impl From<sqlx::Error> for OAuthHttpError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &err
            && db_err.is_unique_violation()
        {
            return Self::new(OAuthErrorCode::UsernameUnavailable, err.to_string());
        }
        Self::server_error(err.to_string())
    }
}

impl From<anyhow::Error> for OAuthHttpError {
    fn from(err: anyhow::Error) -> Self {
        Self::server_error(err.to_string())
    }
}
