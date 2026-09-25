//! Encryption master key generation and decoding.
//!
//! `encryption_master_key` is the 32-byte ChaCha20-Poly1305 key behind
//! at-rest sealing and the gateway accounting journal. It is stored as 64
//! hex characters; this module is its single decoder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rand::Rng;

use super::secrets::SecretsBootstrapError;

pub const ENCRYPTION_MASTER_KEY_BYTES: usize = 32;

#[must_use]
pub fn generate_master_key() -> String {
    let mut key = [0u8; ENCRYPTION_MASTER_KEY_BYTES];
    rand::rng().fill_bytes(&mut key);
    hex::encode(key)
}

pub fn decode_master_key(
    encoded: &str,
) -> Result<[u8; ENCRYPTION_MASTER_KEY_BYTES], SecretsBootstrapError> {
    let raw = hex::decode(encoded.trim()).map_err(|e| {
        SecretsBootstrapError::EncryptionMasterKeyInvalid {
            message: format!("hex decode failed: {e}"),
        }
    })?;
    <[u8; ENCRYPTION_MASTER_KEY_BYTES]>::try_from(raw.as_slice()).map_err(|_e| {
        SecretsBootstrapError::EncryptionMasterKeyInvalid {
            message: format!(
                "expected {ENCRYPTION_MASTER_KEY_BYTES}-byte key, got {}",
                raw.len()
            ),
        }
    })
}
