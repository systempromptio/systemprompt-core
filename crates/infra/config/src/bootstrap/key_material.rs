//! Decoding failures of the encoded key material held in the secrets file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error)]
pub enum KeyMaterialError {
    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("utf-8 decode failed: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("expected {expected}-byte value, got {actual}")]
    Length { expected: usize, actual: usize },
}
