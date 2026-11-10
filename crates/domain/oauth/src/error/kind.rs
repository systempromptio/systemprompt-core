//! RFC 6749 §5.2 error classes the HTTP edge answers with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Error class an [`OauthError`](super::OauthError) maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OauthErrorKind {
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    InvalidToken,
    AccessDenied,
    InvalidClientMetadata,
    UsernameUnavailable,
    EmailExists,
    ExpiredChallenge,
    InvalidCredential,
    AuthenticationFailed,
    NotFound,
    ServerError,
}

impl OauthErrorKind {
    #[must_use]
    pub const fn rfc_code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidClient => "invalid_client",
            Self::InvalidGrant => "invalid_grant",
            Self::InvalidToken => "invalid_token",
            Self::AccessDenied => "access_denied",
            Self::InvalidClientMetadata => "invalid_client_metadata",
            Self::UsernameUnavailable => "username_unavailable",
            Self::EmailExists => "email_exists",
            Self::ExpiredChallenge => "expired_challenge",
            Self::InvalidCredential => "invalid_credential",
            Self::AuthenticationFailed => "authentication_failed",
            Self::NotFound => "not_found",
            Self::ServerError => "server_error",
        }
    }
}
