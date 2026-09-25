//! At-rest hashing for secrets that must be looked up by exact match but
//! must not survive a database read.
//!
//! The deployment's `oauth_at_rest_pepper` is a process-resident HMAC key.
//! Refresh-token identifiers and authorisation codes are stored as the
//! lowercase-hex HMAC-SHA-256 of the raw value under that key, so a leaked
//! database backup or replica snapshot yields opaque digests rather than
//! live credentials.
//!
//! Where a stored secret has to be replayed upstream rather than merely
//! matched — an MCP proxy identity's bearer JWT, say — a digest is no use and
//! [`seal`]/[`open`] encrypt it instead, with ChaCha20-Poly1305 under the
//! deployment's `encryption_master_key`. That is the same key and the same
//! AEAD the gateway accounting journal uses on disk, so a deployment has one
//! at-rest key to hold, not two.
//!
//! Rotation is out of scope here: rolling the pepper invalidates every
//! row hashed under the old key, and rolling the master key invalidates every
//! sealed value. The schema reserves no `pepper_version` column today; a
//! future migration would add one if graceful rotation is required.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const NONCE_BYTES: usize = 12;

/// Why a sealed value could not be produced or read back. Every variant is an
/// operator or key fault; none of them carries the value or the key.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum AtRestCipherError {
    #[error(
        "at-rest encryption requires the `encryption_master_key` secret (32 bytes as 64 hex \
         characters); with `secrets.source: env` it must also be listed in \
         SYSTEMPROMPT_CUSTOM_SECRETS"
    )]
    KeyUnavailable,

    #[error("encryption_master_key is not a 32-byte hex key")]
    KeyInvalid,

    #[error("sealed value is malformed")]
    Malformed,

    #[error("sealed value did not authenticate under encryption_master_key")]
    NotAuthentic,
}

fn cipher() -> Result<ChaCha20Poly1305, AtRestCipherError> {
    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|_e| AtRestCipherError::KeyUnavailable)?;
    let key = secrets
        .get("encryption_master_key")
        .ok_or(AtRestCipherError::KeyUnavailable)?;
    let decoded =
        systemprompt_config::decode_master_key(key).map_err(|_e| AtRestCipherError::KeyInvalid)?;
    Ok(ChaCha20Poly1305::new(&decoded.into()))
}

pub fn seal(plaintext: &str) -> Result<String, AtRestCipherError> {
    let nonce: [u8; NONCE_BYTES] = rand::random();
    let sealed = cipher()?
        .encrypt(&Nonce::from(nonce), plaintext.as_bytes())
        .map_err(|_e| AtRestCipherError::NotAuthentic)?;
    let mut out = Vec::with_capacity(NONCE_BYTES + sealed.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(hex::encode(out))
}

pub fn open(sealed: &str) -> Result<String, AtRestCipherError> {
    let bytes = hex::decode(sealed).map_err(|_e| AtRestCipherError::Malformed)?;
    if bytes.len() <= NONCE_BYTES {
        return Err(AtRestCipherError::Malformed);
    }
    let nonce: [u8; NONCE_BYTES] = bytes[..NONCE_BYTES]
        .try_into()
        .map_err(|_e| AtRestCipherError::Malformed)?;
    let plaintext = cipher()?
        .decrypt(&Nonce::from(nonce), &bytes[NONCE_BYTES..])
        .map_err(|_e| AtRestCipherError::NotAuthentic)?;
    String::from_utf8(plaintext).map_err(|_e| AtRestCipherError::Malformed)
}

#[expect(
    clippy::expect_used,
    reason = "HMAC-SHA256 accepts any key length by construction; new_from_slice cannot fail here"
)]
pub fn hmac_sha256(pepper: &[u8], value: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(pepper).expect("HMAC accepts any key length");
    mac.update(value);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

pub fn hmac_sha256_hex(pepper: &[u8], value: &[u8]) -> String {
    hex::encode(hmac_sha256(pepper, value))
}
