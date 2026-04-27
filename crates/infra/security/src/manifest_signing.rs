use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::Serialize;
use std::sync::OnceLock;
use systemprompt_models::SecretsBootstrap;

pub fn signing_key() -> Result<&'static SigningKey, String> {
    static CELL: OnceLock<SigningKey> = OnceLock::new();
    if let Some(k) = CELL.get() {
        return Ok(k);
    }
    let seed = SecretsBootstrap::manifest_signing_secret_seed()
        .map_err(|e| format!("manifest signing seed unavailable: {e}"))?;
    let key = SigningKey::from_bytes(&seed);
    match CELL.set(key) {
        Ok(()) => Ok(CELL.get().ok_or("key missing after set")?),
        Err(_) => Ok(CELL.get().ok_or("key missing after concurrent set")?),
    }
}

pub fn canonicalize<T: Serialize>(value: &T) -> Result<String, String> {
    serde_jcs::to_string(value).map_err(|e| format!("jcs canonicalize: {e}"))
}

pub fn sign_value<T: Serialize>(value: &T) -> Result<String, String> {
    let canonical = canonicalize(value)?;
    let key = signing_key()?;
    let sig = key.sign(canonical.as_bytes());
    Ok(base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()))
}

pub fn pubkey_b64() -> Result<String, String> {
    let key = signing_key()?;
    let vk: VerifyingKey = key.verifying_key();
    Ok(base64::engine::general_purpose::STANDARD.encode(vk.to_bytes()))
}
