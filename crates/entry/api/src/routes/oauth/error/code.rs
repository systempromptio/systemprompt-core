//! RFC 6749 §5.2 error codes plus the `WebAuthn` / RFC 7591 extensions this
//! server emits, and their default HTTP status mapping.
//!
//! `Display` (via [`OAuthErrorCode::as_str`]) yields the wire string. The
//! default status follows §5.2: token-endpoint errors return 400 except
//! `invalid_client`, which RFC 6749 permits to return 401 to advertise
//! authentication schemes — and so we do. `access_denied`, `invalid_token`,
//! and `authentication_failed` retain 401 because they signal that the
//! *caller* (not the request) was rejected (RFC 6750 §3.1).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthErrorCode {
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    UnauthorizedClient,
    UnsupportedGrantType,
    InvalidScope,
    InvalidToken,
    AccessDenied,
    ServerError,
    TemporarilyUnavailable,
    InvalidClientMetadata,
    AuthenticationFailed,
    RegistrationFailed,
    UsernameUnavailable,
    EmailExists,
    ExpiredChallenge,
    InvalidCredential,
    LinkFailed,
    InvalidTarget,
    NotFound,
}

impl OAuthErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidClient => "invalid_client",
            Self::InvalidGrant => "invalid_grant",
            Self::UnauthorizedClient => "unauthorized_client",
            Self::UnsupportedGrantType => "unsupported_grant_type",
            Self::InvalidScope => "invalid_scope",
            Self::InvalidToken => "invalid_token",
            Self::AccessDenied => "access_denied",
            Self::ServerError => "server_error",
            Self::TemporarilyUnavailable => "temporarily_unavailable",
            Self::InvalidClientMetadata => "invalid_client_metadata",
            Self::AuthenticationFailed => "authentication_failed",
            Self::RegistrationFailed => "registration_failed",
            Self::UsernameUnavailable => "username_unavailable",
            Self::EmailExists => "email_exists",
            Self::ExpiredChallenge => "expired_challenge",
            Self::InvalidCredential => "invalid_credential",
            Self::LinkFailed => "link_failed",
            Self::InvalidTarget => "invalid_target",
            Self::NotFound => "not_found",
        }
    }

    #[must_use]
    pub const fn default_status(self) -> StatusCode {
        match self {
            Self::InvalidRequest
            | Self::InvalidGrant
            | Self::UnauthorizedClient
            | Self::UnsupportedGrantType
            | Self::InvalidScope
            | Self::InvalidClientMetadata
            | Self::ExpiredChallenge
            | Self::InvalidCredential
            | Self::LinkFailed
            | Self::InvalidTarget
            | Self::RegistrationFailed => StatusCode::BAD_REQUEST,
            Self::InvalidClient
            | Self::AccessDenied
            | Self::AuthenticationFailed
            | Self::InvalidToken => StatusCode::UNAUTHORIZED,
            Self::UsernameUnavailable | Self::EmailExists => StatusCode::CONFLICT,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::TemporarilyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}
