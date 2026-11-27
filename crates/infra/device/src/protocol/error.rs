//! Errors of the pure helpers. None of them carries a credential or the value
//! it refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// A credential that cannot be held as the kind it was given as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    #[error("{kind} is empty")]
    Empty { kind: &'static str },
    #[error("{kind} does not have the expected shape")]
    Malformed { kind: &'static str },
}

/// A JWT whose claims cannot be read (the signature is never checked here).
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("token is not three dot-separated segments")]
    Shape,
    #[error("token payload is not base64url")]
    Base64(#[source] base64::DecodeError),
    #[error("token payload is not a JSON object")]
    Claims(#[source] serde_json::Error),
}

/// A manifest public key that is not 32 raw ed25519 bytes in base64.
#[derive(Debug, thiserror::Error)]
pub enum PubkeyError {
    #[error("public key is not base64")]
    Base64(#[source] base64::DecodeError),
    #[error("public key must be 32 bytes, got {0}")]
    Length(usize),
}

/// A URL the harness refuses to send a credential to, or a path that would
/// leave the instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OriginError {
    #[error("not a URL")]
    Parse(#[source] url::ParseError),
    #[error("plain HTTP is only allowed on loopback")]
    Insecure,
    #[error("the URL carries a user name or password")]
    Credentials,
    #[error("the path is not an absolute path on the instance")]
    Path,
    #[error("the URL leaves the instance's origin")]
    Foreign,
}
