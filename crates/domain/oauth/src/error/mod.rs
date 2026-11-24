//! Typed error taxonomy for the systemprompt-oauth domain.
//!
//! Variants enumerate the security-meaningful failure modes encountered
//! throughout the OAuth 2.0 / OIDC, `WebAuthn` and CIMD subsystems. A variant
//! built from another error keeps it as its `source`; a `String` payload is
//! only ever text a caller supplied. [`OauthError::kind`] classifies every
//! variant into the RFC 6749 §5.2 (plus `WebAuthn` / RFC 7591) error class the
//! HTTP edge answers with, so the edge never inspects error text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod http;
mod kind;
mod validation;

pub use kind::OauthErrorKind;
pub use validation::{ClientMetadataError, ValidationError};

use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_traits::{
    AnalyticsProviderError, AuthProviderError, FederatedRefusal, RepositoryError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OauthError {
    #[error("token error: {reason}")]
    TokenInvalid { reason: &'static str },

    #[error("token rejected: {0}")]
    TokenRejected(#[source] systemprompt_models::auth::TokenError),

    #[error("token signed with `{got}`, expected `{expected}`")]
    TokenAlgMismatch { got: String, expected: String },

    #[error("token is missing the `kid` header")]
    TokenMissingKid,

    #[error("token references unknown signing key `{kid}`")]
    TokenUnknownKid { kid: String },

    #[error("expired: {reason}")]
    Expired { reason: &'static str },

    #[error("invalid grant: the enterprise identity cannot be linked: {refusal}")]
    FederatedLinkRefused { refusal: FederatedRefusal },

    #[error("invalid client: {reason}")]
    InvalidClient { reason: &'static str },

    #[error("client not found: {client_id}")]
    ClientNotFound { client_id: ClientId },

    #[error("client not found: DCR client_id '{client_id}' not found in oauth_clients table")]
    DcrClientNotFound { client_id: ClientId },

    #[error("setup token rejected: {0}")]
    SetupTokenRejected(&'static str),

    #[error("username already taken: {username}")]
    UsernameTaken { username: String },

    #[error("email already registered: {email}")]
    EmailRegistered { email: String },

    #[error("user not found: {user_id}")]
    UserNotFound { user_id: UserId },

    #[error("passkey authentication is not available for this account")]
    AuthenticationUnavailable,

    #[error("service accounts cannot obtain an interactive credential")]
    ServiceAccountInteractive,

    #[error("registration state expired or not found")]
    RegistrationStateExpired,

    #[error("authentication challenge expired or not found")]
    ChallengeExpired,

    #[error("webauthn verification failed: {reason}")]
    WebAuthnVerificationFailed { reason: &'static str },

    #[error("webauthn ceremony failed: {0}")]
    WebAuthnCeremony(#[from] webauthn_rs::prelude::WebauthnError),

    #[error("user provider failed while {context}: {source}")]
    UserProvider {
        context: &'static str,
        #[source]
        source: AuthProviderError,
    },

    #[error("session store failed: {0}")]
    SessionStore(#[from] AnalyticsProviderError),

    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("{field} {value:?} is not a valid absolute URL: {source}")]
    InvalidUrl {
        field: String,
        value: String,
        #[source]
        source: url::ParseError,
    },

    #[error("config error: {0}")]
    Config(#[from] systemprompt_models::errors::GlobalConfigError),

    #[error("secrets unavailable: {0}")]
    Secrets(#[from] systemprompt_config::SecretsBootstrapError),

    #[error("at-rest hashing failed: {0}")]
    AtRestHash(#[from] systemprompt_security::AtRestHashError),

    #[error("token signing failed: {0}")]
    Signing(#[from] jsonwebtoken::errors::Error),

    #[error("invalid token lifetime of {seconds} seconds; must be between 1 second and 1 year")]
    InvalidTokenLifetime { seconds: i64 },

    #[error("password hashing failed: {0}")]
    Bcrypt(#[from] bcrypt::BcryptError),

    #[error("stored state is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CIMD HTTP client could not be built: {0}")]
    CimdHttpClient(#[source] reqwest::Error),

    #[error("CIMD metadata fetch from {url} failed: {source}")]
    CimdFetch {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("CIMD client_id {url} is not an allowed outbound target: {source}")]
    CimdTarget {
        url: String,
        #[source]
        source: systemprompt_models::net::OutboundUrlError,
    },

    #[error("CIMD metadata fetch from {url} returned HTTP {status}")]
    CimdStatus { url: String, status: u16 },

    #[error("CIMD metadata from {url} is not valid JSON: {source}")]
    CimdDecode {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("invalid client metadata: {0}")]
    InvalidClientMetadata(#[from] ClientMetadataError),

    #[error("webauthn configuration error: {0}")]
    WebAuthnConfig(&'static str),

    #[error("api_external_url is not a valid URL: {0}")]
    ExternalUrl(#[source] url::ParseError),

    #[error("challenge TTL out of range: {0}")]
    ChallengeTtl(#[source] chrono::OutOfRangeError),

    #[error("internal: {0}")]
    Internal(&'static str),
}

pub type OauthResult<T> = Result<T, OauthError>;

impl From<sqlx::Error> for OauthError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(RepositoryError::from(err))
    }
}

impl From<systemprompt_models::auth::TokenError> for OauthError {
    fn from(err: systemprompt_models::auth::TokenError) -> Self {
        use systemprompt_models::auth::TokenError;
        match err {
            TokenError::UnsupportedAlgorithm { got } => Self::TokenAlgMismatch {
                got,
                expected: "RS256".to_owned(),
            },
            TokenError::KidMismatch => Self::TokenMissingKid,
            TokenError::UnknownKid(kid) => Self::TokenUnknownKid { kid },
            TokenError::Expired => Self::Expired {
                reason: "Token has expired",
            },
            TokenError::InvalidLifetime { seconds } => Self::InvalidTokenLifetime { seconds },
            other => Self::TokenRejected(other),
        }
    }
}

impl From<systemprompt_security::AuthError> for OauthError {
    fn from(err: systemprompt_security::AuthError) -> Self {
        Self::from(systemprompt_models::auth::TokenError::from(err))
    }
}

impl OauthError {
    pub const fn is_unique_violation(&self) -> bool {
        matches!(
            self,
            Self::Repository(RepositoryError::Constraint {
                kind: systemprompt_traits::ConstraintKind::Unique,
                ..
            })
        )
    }
}

impl OauthError {
    #[must_use]
    pub const fn kind(&self) -> OauthErrorKind {
        match self {
            Self::InvalidClient { .. }
            | Self::ClientNotFound { .. }
            | Self::DcrClientNotFound { .. } => OauthErrorKind::InvalidClient,
            Self::InvalidClientMetadata(_) | Self::CimdTarget { .. } => {
                OauthErrorKind::InvalidClientMetadata
            },
            Self::FederatedLinkRefused { .. }
            | Self::TokenInvalid { .. }
            | Self::Expired { .. }
            | Self::SetupTokenRejected(_) => OauthErrorKind::InvalidGrant,
            Self::TokenRejected(_)
            | Self::TokenAlgMismatch { .. }
            | Self::TokenMissingKid
            | Self::TokenUnknownKid { .. } => OauthErrorKind::InvalidToken,
            Self::Validation(_) | Self::InvalidUrl { .. } => OauthErrorKind::InvalidRequest,
            Self::ServiceAccountInteractive => OauthErrorKind::AccessDenied,
            Self::UsernameTaken { .. } => OauthErrorKind::UsernameUnavailable,
            Self::EmailRegistered { .. } => OauthErrorKind::EmailExists,
            Self::UserNotFound { .. } => OauthErrorKind::NotFound,
            Self::AuthenticationUnavailable => OauthErrorKind::AuthenticationFailed,
            Self::RegistrationStateExpired | Self::ChallengeExpired => {
                OauthErrorKind::ExpiredChallenge
            },
            Self::WebAuthnVerificationFailed { .. } | Self::WebAuthnCeremony(_) => {
                OauthErrorKind::InvalidCredential
            },
            Self::UserProvider { .. }
            | Self::SessionStore(_)
            | Self::Repository(_)
            | Self::Config(_)
            | Self::Secrets(_)
            | Self::AtRestHash(_)
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
