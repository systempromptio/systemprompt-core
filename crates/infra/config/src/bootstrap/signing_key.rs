//! The RS256 token key's rotation state in the secrets document.
//!
//! The active key is `signing_key_pem` (base64 of a PKCS#8 private-key PEM).
//! After a rotation the key it replaced is kept for verification only, as
//! `signing_key_previous_public_pem` (base64 of an SPKI public-key PEM — no
//! private material) together with `signing_key_previous_expires_at`
//! (RFC 3339). The two fields are set together or not at all.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use base64::Engine;
use chrono::{DateTime, Utc};
use systemprompt_manifest::secrets::Secrets;

use super::secrets::SecretsBootstrapError;
use crate::error::{ConfigError, ConfigResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviousSigningKey {
    pub public_pem: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningKeyRotation {
    pub active_private_pem: String,
    pub previous: PreviousSigningKey,
}

pub fn decode_previous_signing_key(
    secrets: &Secrets,
) -> Result<Option<PreviousSigningKey>, SecretsBootstrapError> {
    let (pem, expires_at) = match (
        secrets.signing_key_previous_public_pem.as_deref(),
        secrets.signing_key_previous_expires_at.as_deref(),
    ) {
        (None, None) => return Ok(None),
        (Some(pem), Some(expires_at)) => (pem, expires_at),
        _ => return Err(SecretsBootstrapError::SigningKeyPreviousIncomplete),
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(pem.trim())
        .map_err(|e| SecretsBootstrapError::SigningKeyPreviousInvalid(e.into()))?;
    let public_pem = String::from_utf8(bytes)
        .map_err(|e| SecretsBootstrapError::SigningKeyPreviousInvalid(e.into()))?;
    let expires_at = DateTime::parse_from_rfc3339(expires_at.trim())
        .map_err(SecretsBootstrapError::SigningKeyPreviousExpiryInvalid)?
        .with_timezone(&Utc);
    Ok(Some(PreviousSigningKey {
        public_pem,
        expires_at,
    }))
}

pub fn persist_signing_key_rotation(
    path: &Path,
    rotation: &SigningKeyRotation,
) -> ConfigResult<()> {
    let engine = base64::engine::general_purpose::STANDARD;
    let content = std::fs::read_to_string(path)?;
    // JSON: opaque secrets doc — must preserve unknown keys
    let mut value: serde_json::Value = serde_json::from_str(&content)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ConfigError::SecretsFileNotObject {
            path: path.to_path_buf(),
        })?;
    object.insert(
        "signing_key_pem".to_owned(),
        serde_json::Value::String(engine.encode(&rotation.active_private_pem)),
    );
    object.insert(
        "signing_key_previous_public_pem".to_owned(),
        serde_json::Value::String(engine.encode(&rotation.previous.public_pem)),
    );
    object.insert(
        "signing_key_previous_expires_at".to_owned(),
        serde_json::Value::String(rotation.previous.expires_at.to_rfc3339()),
    );
    let serialized = serde_json::to_string_pretty(&value)?;
    crate::private_file::write_private_atomic(path, serialized.as_bytes())?;
    Ok(())
}
