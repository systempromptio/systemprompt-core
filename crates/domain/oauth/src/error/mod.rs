//! Typed error taxonomy for the systemprompt-oauth domain.
//!
//! Variants enumerate the security-meaningful failure modes encountered
//! throughout the OAuth 2.0 / OIDC, `WebAuthn` and CIMD subsystems. A variant
//! built from another error keeps it as its `source`; a `String` payload is
//! only ever text this crate authors. [`OauthError::kind`] classifies every
//! variant into the RFC 6749 §5.2 (plus `WebAuthn` / RFC 7591) error class the
//! HTTP edge answers with, so the edge never inspects error text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod kind;

pub use kind::OauthErrorKind;

use systemprompt_traits::{AnalyticsProviderError, AuthProviderError, RepositoryError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OauthError {
    #[error("token error: {0}")]
    TokenInvalid(String),

    #[error("token rejected: {0}")]
    TokenRejected(#[source] systemprompt_security::AuthError),

    #[error("token signed with `{got}`, expected `{expected}`")]
    TokenAlgMismatch { got: String, expected: String },

    #[error("token is missing the `kid` header")]
    TokenMissingKid,

    #[error("token references unknown signing key `{kid}`")]
    TokenUnknownKid { kid: String },

    #[error("token not found: {0}")]
    TokenNotFound(String),

    #[error("authorization code not found: {0}")]
    CodeNotFound(String),

    #[error("expired: {0}")]
    Expired(String),

    #[error("PKCE challenge mismatch: {0}")]
    PkceMismatch(String),

    #[error("invalid grant: {0}")]
    InvalidGrant(String),

    #[error("invalid client: {0}")]
    InvalidClient(String),

    #[error("client not found: {0}")]
    ClientNotFound(String),

    #[error("setup token rejected: {0}")]
    SetupTokenRejected(&'static str),

    #[error("username already taken: {0}")]
    UsernameTaken(String),

    #[error("email already registered: {0}")]
    EmailRegistered(String),

    #[error("user not found: {0}")]
    UserNotFound(String),

    #[error("passkey authentication is not available for this account")]
    AuthenticationUnavailable,

    #[error("registration state expired or not found")]
    RegistrationStateExpired,

    #[error("authentication challenge expired or not found")]
    ChallengeExpired,

    #[error("webauthn verification failed: {0}")]
    WebAuthnVerificationFailed(String),

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
    Validation(String),

    #[error("{field} {value:?} is not a valid absolute URL: {source}")]
    InvalidUrl {
        field: String,
        value: String,
        #[source]
        source: url::ParseError,
    },

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("config error: {0}")]
    Config(#[from] systemprompt_models::errors::ConfigError),

    #[error("secrets unavailable: {0}")]
    Secrets(#[from] systemprompt_config::SecretsBootstrapError),

    #[error("signing key unavailable: {0}")]
    SigningKey(#[from] systemprompt_security::keys::TokenAuthorityError),

    #[error("token signing failed: {0}")]
    Signing(#[from] jsonwebtoken::errors::Error),

    #[error("invalid token lifetime of {seconds} seconds; must be between 1 second and 1 year")]
    InvalidTokenLifetime { seconds: i64 },

    #[error("password hashing failed: {0}")]
    Bcrypt(#[from] bcrypt::BcryptError),

    #[error("stored state is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("user id {user_id} is not a valid UUID: {source}")]
    InvalidUserId {
        user_id: systemprompt_identifiers::UserId,
        #[source]
        source: uuid::Error,
    },

    #[error("CIMD HTTP client could not be built: {0}")]
    CimdHttpClient(#[source] reqwest::Error),

    #[error("CIMD metadata fetch from {url} failed: {source}")]
    CimdFetch {
        url: String,
        #[source]
        source: reqwest::Error,
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
    InvalidClientMetadata(String),

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

impl From<systemprompt_security::AuthError> for OauthError {
    fn from(err: systemprompt_security::AuthError) -> Self {
        use jsonwebtoken::errors::ErrorKind;
        use systemprompt_security::AuthError;
        match err {
            AuthError::UnsupportedAlgorithm { got } => Self::TokenAlgMismatch {
                got,
                expected: "RS256".to_owned(),
            },
            AuthError::MissingKid => Self::TokenMissingKid,
            AuthError::UnknownKid(kid) => Self::TokenUnknownKid { kid },
            AuthError::InvalidToken(e) if matches!(e.kind(), ErrorKind::ExpiredSignature) => {
                Self::Expired("Token has expired".to_owned())
            },
            other => Self::TokenRejected(other),
        }
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
