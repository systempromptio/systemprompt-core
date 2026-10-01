//! Manifest signing seed generation, decoding, and persistence.
//!
//! The manifest signing key is a 32-byte secret used by the
//! bridge/manifest pipeline to detach-sign module manifests. This
//! module owns its base64 encoding and persists rotated seeds back into
//! the secrets file through an owner-only atomic write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use base64::Engine;
use rand::Rng;

use super::key_material::KeyMaterialError;
use super::secrets::SecretsBootstrapError;
use crate::error::{ConfigError, ConfigResult};

pub const MANIFEST_SIGNING_SEED_BYTES: usize = 32;

#[must_use]
pub fn generate_seed() -> [u8; MANIFEST_SIGNING_SEED_BYTES] {
    let mut seed = [0u8; MANIFEST_SIGNING_SEED_BYTES];
    rand::rng().fill_bytes(&mut seed);
    seed
}

pub fn decode_seed(
    encoded: &str,
) -> Result<[u8; MANIFEST_SIGNING_SEED_BYTES], SecretsBootstrapError> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| SecretsBootstrapError::ManifestSeedInvalid(e.into()))?;
    if raw.len() != MANIFEST_SIGNING_SEED_BYTES {
        return Err(SecretsBootstrapError::ManifestSeedInvalid(
            KeyMaterialError::Length {
                expected: MANIFEST_SIGNING_SEED_BYTES,
                actual: raw.len(),
            },
        ));
    }
    let mut out = [0u8; MANIFEST_SIGNING_SEED_BYTES];
    out.copy_from_slice(&raw);
    Ok(out)
}

pub fn persist_seed(path: &Path, seed: &[u8; MANIFEST_SIGNING_SEED_BYTES]) -> ConfigResult<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(seed);
    let content = std::fs::read_to_string(path)?;
    // JSON: opaque secrets doc — must preserve unknown keys
    let mut value: serde_json::Value = serde_json::from_str(&content)?;
    let object = value.as_object_mut().ok_or_else(|| {
        ConfigError::other(format!(
            "secrets file root is not a JSON object: {}",
            path.display()
        ))
    })?;
    object.insert(
        "manifest_signing_secret_seed".to_owned(),
        serde_json::Value::String(encoded),
    );
    let serialized = serde_json::to_string_pretty(&value)?;
    crate::private_file::write_private_atomic(path, serialized.as_bytes())?;
    Ok(())
}
