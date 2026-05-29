//! Typed error taxonomy for the systemprompt-oauth domain.
//!
//! Variants enumerate the security-meaningful failure modes encountered
//! throughout the OAuth 2.0 / OIDC, `WebAuthn` and CIMD subsystems. Concrete
//! `#[from]` adapters route `sqlx`, `std::io`, `url`, `serde_json`, and
//! `webauthn`/`bcrypt`/`jsonwebtoken` errors into the appropriate variant
//! so callers can match on a single `OauthError` enum.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum OauthError {
    #[error("provider error: {0}")]
    Provider(String),

    #[error("token error: {0}")]
    TokenInvalid(String),

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

    #[error("session error: {0}")]
    Session(String),

    #[error("webauthn error: {0}")]
    WebAuthn(String),

    #[error("username already taken: {0}")]
    UsernameTaken(String),

    #[error("email already registered: {0}")]
    EmailRegistered(String),

    #[error("user not found: {0}")]
    UserNotFound(String),

    #[error("registration state expired or not found")]
    RegistrationStateExpired,

    #[error("webauthn verification failed: {0}")]
    WebAuthnVerificationFailed(String),

    #[error("user error: {0}")]
    User(String),

    #[error("repository error: {0}")]
    Repository(#[from] sqlx::Error),

    #[error("database repository error: {0}")]
    DatabaseRepository(#[from] systemprompt_database::RepositoryError),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("internal: {0}")]
    Internal(String),
}

pub type OauthResult<T> = Result<T, OauthError>;

impl From<webauthn_rs::prelude::WebauthnError> for OauthError {
    fn from(err: webauthn_rs::prelude::WebauthnError) -> Self {
        Self::WebAuthnVerificationFailed(err.to_string())
    }
}

impl From<bcrypt::BcryptError> for OauthError {
    fn from(err: bcrypt::BcryptError) -> Self {
        Self::Crypto(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for OauthError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        Self::TokenInvalid(err.to_string())
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
            other => Self::TokenInvalid(other.to_string()),
        }
    }
}

impl From<serde_json::Error> for OauthError {
    fn from(err: serde_json::Error) -> Self {
        Self::Validation(format!("json parse: {err}"))
    }
}

impl From<systemprompt_models::errors::ConfigError> for OauthError {
    fn from(err: systemprompt_models::errors::ConfigError) -> Self {
        Self::Config(err.to_string())
    }
}

impl From<systemprompt_config::SecretsBootstrapError> for OauthError {
    fn from(err: systemprompt_config::SecretsBootstrapError) -> Self {
        Self::Config(err.to_string())
    }
}
