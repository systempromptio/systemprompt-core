//! The canonical `OauthError` → [`ApiError`] mapping every HTTP edge uses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::api::ApiError;

use super::{OauthError, OauthErrorKind};

impl From<OauthError> for ApiError {
    fn from(err: OauthError) -> Self {
        if matches!(
            err,
            OauthError::CodeNotFound(_)
                | OauthError::TokenNotFound(_)
                | OauthError::ClientNotFound(_)
                | OauthError::UserNotFound(_)
        ) {
            return Self::not_found(err.to_string());
        }
        match err.kind() {
            OauthErrorKind::InvalidRequest
            | OauthErrorKind::InvalidClientMetadata
            | OauthErrorKind::ExpiredChallenge
            | OauthErrorKind::InvalidCredential => Self::bad_request(err.to_string()),
            OauthErrorKind::UsernameUnavailable | OauthErrorKind::EmailExists => {
                Self::conflict(err.to_string())
            },
            OauthErrorKind::InvalidClient => Self::unauthorized("Client authentication failed"),
            OauthErrorKind::AuthenticationFailed => Self::unauthorized("Authentication failed"),
            OauthErrorKind::InvalidGrant
            | OauthErrorKind::InvalidToken
            | OauthErrorKind::AccessDenied => Self::unauthorized(err.to_string()),
            OauthErrorKind::NotFound => Self::not_found(err.to_string()),
            OauthErrorKind::ServerError => Self::internal("Authorization operation failed", err),
        }
    }
}
