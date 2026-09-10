//! Ed25519 signing of bridge manifests.
//!
//! The signing key is derived from a 32-byte seed loaded by
//! [`systemprompt_config::SecretsBootstrap`]. The seed is cached in a
//! process-wide [`OnceLock`] so the key derivation runs at most once per
//! process. Manifests are canonicalised via JSON Canonicalization Scheme
//! (RFC 8785) before signing so that semantically-equivalent payloads
//! produce identical signatures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use systemprompt_config::SecretsBootstrap;

use crate::error::{ManifestSigningError, ManifestSigningResult};

const ED25519_PUBLIC_KEY_LEN: usize = 32;
const ED25519_SIGNATURE_LEN: usize = 64;
const KEY_ID_HEX_CHARS: usize = 16;

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

pub fn sign_bytes(payload: &[u8]) -> ManifestSigningResult<String> {
    let key = signing_key()?;
    let sig = key.sign(payload);
    Ok(base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()))
}

pub fn pubkey_b64() -> ManifestSigningResult<String> {
    let key = signing_key()?;
    let vk: VerifyingKey = key.verifying_key();
    Ok(base64::engine::general_purpose::STANDARD.encode(vk.to_bytes()))
}

pub fn sign_with_seed(seed: &[u8; 32], payload: &[u8]) -> String {
    let key = SigningKey::from_bytes(seed);
    base64::engine::general_purpose::STANDARD.encode(key.sign(payload).to_bytes())
}

pub fn pubkey_b64_from_seed(seed: &[u8; 32]) -> String {
    let vk = SigningKey::from_bytes(seed).verifying_key();
    base64::engine::general_purpose::STANDARD.encode(vk.to_bytes())
}

pub fn verify_with_pubkey(
    pubkey_b64: &str,
    payload: &[u8],
    sig_b64: &str,
) -> ManifestSigningResult<()> {
    let key_bytes: [u8; ED25519_PUBLIC_KEY_LEN] =
        decode_fixed(pubkey_b64, "ed25519 public key", ED25519_PUBLIC_KEY_LEN)?
            .try_into()
            .map_err(|_e| ManifestSigningError::KeyMissing)?;
    let sig_bytes: [u8; ED25519_SIGNATURE_LEN] =
        decode_fixed(sig_b64, "ed25519 signature", ED25519_SIGNATURE_LEN)?
            .try_into()
            .map_err(|_e| ManifestSigningError::SignatureInvalid)?;

    let verifying_key =
        VerifyingKey::from_bytes(&key_bytes).map_err(|e| ManifestSigningError::InvalidBase64 {
            field: "ed25519 public key",
            message: e.to_string(),
        })?;
    verifying_key
        .verify(payload, &Signature::from_bytes(&sig_bytes))
        .map_err(|_e| ManifestSigningError::SignatureInvalid)
}

pub fn canonical_manifest_bytes<T: Serialize>(manifest: &T) -> ManifestSigningResult<Vec<u8>> {
    canonicalize(manifest).map(String::into_bytes)
}

pub fn key_id_for_pubkey(pubkey_b64: &str) -> String {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(pubkey_b64)
        .unwrap_or_else(|_e| pubkey_b64.as_bytes().to_vec());
    let digest = hex::encode(Sha256::digest(&raw));
    digest[..KEY_ID_HEX_CHARS].to_owned()
}

fn decode_fixed(
    value: &str,
    field: &'static str,
    expected: usize,
) -> ManifestSigningResult<Vec<u8>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|e| ManifestSigningError::InvalidBase64 {
            field,
            message: e.to_string(),
        })?;
    if bytes.len() == expected {
        Ok(bytes)
    } else {
        Err(ManifestSigningError::InvalidKeyLength {
            field,
            expected,
            actual: bytes.len(),
        })
    }
}
