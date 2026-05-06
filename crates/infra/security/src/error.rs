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
}

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("jwt encoding failed: {0}")]
    Encoding(#[from] jsonwebtoken::errors::Error),
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
