//! Error types raised by the security infrastructure.
//!
//! Public APIs in this crate return `thiserror`-derived error enums:
//!
//! - [`AuthError`] — request validation, JWT decoding, claim extraction.
//! - [`JwtError`] — JWT minting (admin tokens, session tokens).
//! - [`ManifestSigningError`] — Ed25519 signing of bridge manifests.
//!
//! All three implement `std::error::Error` and can be composed into larger
//! `thiserror` enums via `#[from]`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("missing authorization header")]
    MissingAuthorization,

    #[error("invalid JWT token: {0}")]
    InvalidToken(#[source] jsonwebtoken::errors::Error),

    #[error("missing session_id in token")]
    MissingSessionId,

    #[error("hook token: missing or non-`hook` audience")]
    HookAudienceMissing,

    #[error("hook token: required scope `{0}` not present")]
    HookScopeMissing(&'static str),

    #[error("hook token: missing `plugin_id` claim")]
    HookPluginIdMissing,

    #[error(
        "hook token: plugin_id `{actual}` in claim does not match request plugin_id `{expected}`"
    )]
    HookPluginIdMismatch { expected: String, actual: String },

    #[error("token has unsupported algorithm; only RS256 is accepted")]
    UnsupportedAlgorithm,

    #[error("token is missing `kid` header")]
    MissingKid,

    #[error("token `kid` `{0}` does not match any known signing key")]
    UnknownKid(String),

    #[error("signing key lookup failed: {0}")]
    KeyLookup(String),

    #[error("issuer `{0}` is not trusted")]
    UntrustedIssuer(String),

    #[error("JWKS fetch failed for issuer `{issuer}`: {source}")]
    JwksFetch {
        issuer: String,
        #[source]
        source: crate::keys::JwksClientError,
    },

    #[error("token `act` delegation chain exceeds maximum depth of {max} (got {depth})")]
    ActChainTooDeep { depth: usize, max: usize },
}

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("jwt encoding failed: {0}")]
    Encoding(#[from] jsonwebtoken::errors::Error),

    #[error("jwt signing key unavailable: {0}")]
    Signing(String),
}

#[derive(Debug, Error)]
pub enum ManifestSigningError {
    #[error("manifest signing seed unavailable: {0}")]
    SeedUnavailable(String),

    #[error("jcs canonicalize: {0}")]
    Canonicalize(String),

    #[error("signing key missing after initialization")]
    KeyMissing,
}

pub type AuthResult<T> = Result<T, AuthError>;

pub type JwtResult<T> = Result<T, JwtError>;

pub type ManifestSigningResult<T> = Result<T, ManifestSigningError>;
