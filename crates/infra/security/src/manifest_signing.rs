use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use systemprompt_models::SecretsBootstrap;

const DOMAIN_SEPARATOR: &[u8] = b"systemprompt-cowork-manifest-ed25519-v1";

pub fn signing_key() -> Result<&'static SigningKey, String> {
    static CELL: OnceLock<SigningKey> = OnceLock::new();
    if let Some(k) = CELL.get() {
        return Ok(k);
    }
    let secret =
        SecretsBootstrap::jwt_secret().map_err(|e| format!("jwt secret unavailable: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_SEPARATOR);
    hasher.update(secret.as_bytes());
    let seed: [u8; 32] = hasher.finalize().into();
    let key = SigningKey::from_bytes(&seed);
    match CELL.set(key) {
        Ok(()) => Ok(CELL.get().ok_or("key missing after set")?),
        Err(_) => Ok(CELL.get().ok_or("key missing after concurrent set")?),
    }
}

pub fn sign_payload(bytes: &[u8]) -> Result<String, String> {
    let key = signing_key()?;
    let sig = key.sign(bytes);
    Ok(base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()))
}

pub fn pubkey_b64() -> Result<String, String> {
    let key = signing_key()?;
    let vk: VerifyingKey = key.verifying_key();
    Ok(base64::engine::general_purpose::STANDARD.encode(vk.to_bytes()))
}
