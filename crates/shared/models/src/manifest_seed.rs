use anyhow::{Context, Result};
use base64::Engine;
use rand::RngCore;
use std::path::Path;

use crate::secrets_bootstrap::SecretsBootstrapError;

pub const MANIFEST_SIGNING_SEED_BYTES: usize = 32;

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
        .map_err(|e| SecretsBootstrapError::ManifestSeedInvalid {
            message: format!("base64 decode failed: {e}"),
        })?;
    if raw.len() != MANIFEST_SIGNING_SEED_BYTES {
        return Err(SecretsBootstrapError::ManifestSeedInvalid {
            message: format!(
                "expected {MANIFEST_SIGNING_SEED_BYTES}-byte seed, got {}",
                raw.len()
            ),
        });
    }
    let mut out = [0u8; MANIFEST_SIGNING_SEED_BYTES];
    out.copy_from_slice(&raw);
    Ok(out)
}

pub fn persist_seed(path: &Path, seed: &[u8; MANIFEST_SIGNING_SEED_BYTES]) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(seed);
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read secrets file: {}", path.display()))?;
    let mut value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse secrets JSON at {}", path.display()))?;
    let object = value
        .as_object_mut()
        .context("secrets file root is not a JSON object")?;
    object.insert(
        "manifest_signing_secret_seed".to_owned(),
        serde_json::Value::String(encoded),
    );
    let serialized =
        serde_json::to_string_pretty(&value).context("Failed to serialize updated secrets")?;
    write_atomic(path, serialized.as_bytes())
        .with_context(|| format!("Failed to write secrets file: {}", path.display()))?;
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().map_or_else(
        || "secrets.json".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let tmp = parent.join(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
