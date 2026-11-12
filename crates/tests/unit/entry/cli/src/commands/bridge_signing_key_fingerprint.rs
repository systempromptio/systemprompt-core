//! `admin bridge signing-key-fingerprint`: the fingerprint is the lowercase
//! hex SHA-256 of the base64 pubkey `/v1/bridge/pubkey` serves.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use base64::Engine;
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};
use systemprompt_cli::admin::bridge::signing_key_fingerprint::{
    key_fingerprint, manifest_pubkey_b64,
};

#[test]
fn pubkey_is_the_base64_ed25519_verifying_key_of_the_seed() {
    let seed = [7u8; 32];
    let expected = base64::engine::general_purpose::STANDARD
        .encode(SigningKey::from_bytes(&seed).verifying_key().to_bytes());
    assert_eq!(manifest_pubkey_b64(&seed), expected);
}

#[test]
fn fingerprint_is_lowercase_sha256_hex_of_the_trimmed_pubkey() {
    let pubkey = manifest_pubkey_b64(&[7u8; 32]);
    let expected: String = Sha256::digest(pubkey.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(key_fingerprint(&pubkey), expected);
    assert_eq!(key_fingerprint(&format!("  {pubkey}\n")), expected);
    assert_eq!(expected.len(), 64);
}
