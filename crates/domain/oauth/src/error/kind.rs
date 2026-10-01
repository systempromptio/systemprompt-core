//! RFC 6749 §5.2 error classes (plus the `WebAuthn` / RFC 7591 extensions)
//! that every [`OauthError`] variant belongs to.
//!
//! The classification is exhaustive over the variants, so a new variant must
//! choose its class here and the HTTP edge maps the class, never the text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OauthError;

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

impl OauthError {
    #[must_use]
    pub const fn kind(&self) -> OauthErrorKind {
        match self {
            Self::InvalidClient(_) | Self::ClientNotFound(_) => OauthErrorKind::InvalidClient,
            Self::InvalidClientMetadata(_) => OauthErrorKind::InvalidClientMetadata,
            Self::InvalidGrant(_)
            | Self::CodeNotFound(_)
            | Self::TokenNotFound(_)
            | Self::TokenInvalid(_)
            | Self::PkceMismatch(_)
            | Self::Expired(_)
            | Self::SetupTokenRejected(_) => OauthErrorKind::InvalidGrant,
            Self::TokenRejected(_)
            | Self::TokenAlgMismatch { .. }
            | Self::TokenMissingKid
            | Self::TokenUnknownKid { .. } => OauthErrorKind::InvalidToken,
            Self::Validation(_) | Self::InvalidUrl { .. } => OauthErrorKind::InvalidRequest,
            Self::Unauthorized(_) => OauthErrorKind::AccessDenied,
            Self::UsernameTaken(_) => OauthErrorKind::UsernameUnavailable,
            Self::EmailRegistered(_) => OauthErrorKind::EmailExists,
            Self::UserNotFound(_) => OauthErrorKind::NotFound,
            Self::AuthenticationUnavailable => OauthErrorKind::AuthenticationFailed,
            Self::RegistrationStateExpired | Self::ChallengeExpired => {
                OauthErrorKind::ExpiredChallenge
            },
            Self::WebAuthnVerificationFailed(_) | Self::WebAuthnCeremony(_) => {
                OauthErrorKind::InvalidCredential
            },
            Self::UserProvider { .. }
            | Self::SessionStore(_)
            | Self::Repository(_)
            | Self::Config(_)
            | Self::Secrets(_)
            | Self::SigningKey(_)
            | Self::Signing(_)
            | Self::InvalidTokenLifetime { .. }
            | Self::Bcrypt(_)
            | Self::Json(_)
            | Self::CimdHttpClient(_)
            | Self::CimdFetch { .. }
            | Self::CimdStatus { .. }
            | Self::CimdDecode { .. }
            | Self::WebAuthnConfig(_)
            | Self::ExternalUrl(_)
            | Self::ChallengeTtl(_)
            | Self::Internal(_) => OauthErrorKind::ServerError,
        }
    }
}
