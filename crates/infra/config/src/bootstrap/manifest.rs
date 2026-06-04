//! Manifest signing seed generation, decoding, and persistence.
//!
//! The manifest signing key is a 32-byte secret used by the
//! bridge/manifest pipeline to detach-sign module manifests. This
//! module owns its base64 encoding and the atomic-write helper that
//! persists rotated seeds back into the secrets file.

use std::path::Path;

use base64::Engine;
use rand::Rng;

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

pub fn persist_seed(path: &Path, seed: &[u8; MANIFEST_SIGNING_SEED_BYTES]) -> ConfigResult<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(seed);
    let content = std::fs::read_to_string(path)?;
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
    write_atomic(path, serialized.as_bytes())?;
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

/// Detects a read-only profile mount before writing the manifest signing seed,
/// so callers can degrade gracefully rather than failing with `EROFS`.
#[must_use]
pub(super) fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(".sp-write-probe");
    if std::fs::write(&probe, b"").is_err() {
        return false;
    }
    // Best-effort cleanup of the probe file; failure to remove it does not
    // change the fact that the directory is writable.
    drop(std::fs::remove_file(&probe));
    true
}
