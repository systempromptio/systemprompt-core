//! Ed25519 signing of bridge manifests.
//!
//! The signing key is derived from a 32-byte seed loaded by
//! [`systemprompt_config::SecretsBootstrap`]. The seed is cached in a
//! process-wide [`OnceLock`] so the key derivation runs at most once per
//! process. Manifests are canonicalised via JSON Canonicalization Scheme
//! (RFC 8785) before signing so that semantically-equivalent payloads
//! produce identical signatures.

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::Serialize;
use std::sync::OnceLock;
use systemprompt_config::SecretsBootstrap;

use crate::error::{ManifestSigningError, ManifestSigningResult};

pub fn signing_key() -> ManifestSigningResult<&'static SigningKey> {
    static CELL: OnceLock<SigningKey> = OnceLock::new();
    if let Some(k) = CELL.get() {
        return Ok(k);
    }
    let seed = SecretsBootstrap::manifest_signing_secret_seed()
        .map_err(|e| ManifestSigningError::SeedUnavailable(e.to_string()))?;
    let key = SigningKey::from_bytes(&seed);
    drop(CELL.set(key));
    CELL.get().ok_or(ManifestSigningError::KeyMissing)
}

pub fn canonicalize<T: Serialize>(value: &T) -> ManifestSigningResult<String> {
    serde_jcs::to_string(value).map_err(|e| ManifestSigningError::Canonicalize(e.to_string()))
}

pub fn sign_value<T: Serialize>(value: &T) -> ManifestSigningResult<String> {
    let canonical = canonicalize(value)?;
    let key = signing_key()?;
    let sig = key.sign(canonical.as_bytes());
    Ok(base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()))
}

pub fn pubkey_b64() -> ManifestSigningResult<String> {
    let key = signing_key()?;
    let vk: VerifyingKey = key.verifying_key();
    Ok(base64::engine::general_purpose::STANDARD.encode(vk.to_bytes()))
}
