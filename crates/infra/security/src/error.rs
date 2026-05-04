//! Error types raised by the security infrastructure.
//!
//! Public APIs in this crate return `thiserror`-derived error enums:
//!
//! - [`AuthError`] — request validation, JWT decoding, claim extraction.
//! - [`JwtError`] — JWT minting (admin tokens, session tokens).
//! - [`ManifestSigningError`] — Ed25519 signing of cowork manifests.
//!
//! All three implement `std::error::Error` and can be composed into larger
//! `thiserror` enums via `#[from]`.

use thiserror::Error;

/// Failures produced while validating an HTTP request's authentication
/// material.
#[derive(Debug, Error)]
pub enum AuthError {
    /// The request did not carry a usable bearer token.
    #[error("missing authorization header")]
    MissingAuthorization,

    /// The supplied JWT failed signature, audience, issuer, or expiry
    /// validation.
    #[error("invalid JWT token: {0}")]
    InvalidToken(#[source] jsonwebtoken::errors::Error),

    /// The JWT decoded successfully but did not carry a `session_id` claim.
    #[error("missing session_id in token")]
    MissingSessionId,
}

/// Failures produced while minting JWTs.
#[derive(Debug, Error)]
pub enum JwtError {
    /// The underlying `jsonwebtoken` encoder rejected the claim set or key.
    #[error("jwt encoding failed: {0}")]
    Encoding(#[from] jsonwebtoken::errors::Error),
}

/// Failures produced while signing manifests with the cowork signing key.
#[derive(Debug, Error)]
pub enum ManifestSigningError {
    /// The signing seed could not be retrieved from the secrets bootstrap
    /// (typically because secrets have not been loaded yet).
    #[error("manifest signing seed unavailable: {0}")]
    SeedUnavailable(String),

    /// JSON Canonicalization Scheme (RFC 8785) serialization failed.
    #[error("jcs canonicalize: {0}")]
    Canonicalize(String),

    /// The `OnceLock` holding the signing key was not populated after a
    /// successful set; this is unreachable in practice but surfaced as a
    /// typed error rather than a panic.
    #[error("signing key missing after initialization")]
    KeyMissing,
}

/// Convenience [`Result`] alias parameterised on [`AuthError`].
pub type AuthResult<T> = Result<T, AuthError>;

/// Convenience [`Result`] alias parameterised on [`JwtError`].
pub type JwtResult<T> = Result<T, JwtError>;

/// Convenience [`Result`] alias parameterised on [`ManifestSigningError`].
pub type ManifestSigningResult<T> = Result<T, ManifestSigningError>;
