//! The RS256 token key's rotation state in the secrets document.
//!
//! The active key is `signing_key_pem` (base64 of a PKCS#8 private-key PEM).
//! Keys it replaced are kept for verification only, as one string holding a
//! JSON array so the list travels like every other secret (file, env, Vault).
//! Each entry carries the key's `kid`, its SPKI public-key PEM (no private
//! material) and its RFC 3339 expiry. A promotion appends the outgoing key and
//! prunes expired entries; a revoking promotion empties the list. A staged
//! key (base64 PKCS#8) verifies but does not sign yet: staging rolls it out
//! to every verifier before a promotion makes it the signer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_manifest::secrets::Secrets;

use super::secrets::SecretsBootstrapError;
use crate::error::{ConfigError, ConfigResult};

pub const SIGNING_KEY_PREVIOUS_KEYS: &str = "signing_key_previous_keys";
pub const SIGNING_KEY_STAGED_PEM: &str = "signing_key_staged_pem";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviousSigningKey {
    pub kid: String,
    pub public_pem: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningKeyState {
    pub active_private_pem: String,
    pub staged_private_pem: Option<String>,
    pub previous: Vec<PreviousSigningKey>,
}

pub fn decode_previous_signing_keys(
    secrets: &Secrets,
) -> Result<Vec<PreviousSigningKey>, SecretsBootstrapError> {
    secrets.signing_key_previous_keys.as_deref().map_or_else(
        || Ok(Vec::new()),
        |raw| serde_json::from_str(raw).map_err(SecretsBootstrapError::SigningKeyPreviousInvalid),
    )
}

pub fn persist_signing_key_state(path: &Path, state: &SigningKeyState) -> ConfigResult<()> {
    let content = std::fs::read_to_string(path)?;
    // JSON: opaque secrets doc — must preserve unknown keys
    let mut value: serde_json::Value = serde_json::from_str(&content)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ConfigError::SecretsFileNotObject {
            path: path.to_path_buf(),
        })?;
    let engine = base64::engine::general_purpose::STANDARD;
    object.insert(
        "signing_key_pem".to_owned(),
        serde_json::Value::String(engine.encode(&state.active_private_pem)),
    );
    match &state.staged_private_pem {
        Some(pem) => {
            object.insert(
                SIGNING_KEY_STAGED_PEM.to_owned(),
                serde_json::Value::String(engine.encode(pem)),
            );
        },
        None => {
            object.remove(SIGNING_KEY_STAGED_PEM);
        },
    }
    if state.previous.is_empty() {
        object.remove(SIGNING_KEY_PREVIOUS_KEYS);
    } else {
        object.insert(
            SIGNING_KEY_PREVIOUS_KEYS.to_owned(),
            serde_json::Value::String(serde_json::to_string(&state.previous)?),
        );
    }
    let serialized = serde_json::to_string_pretty(&value)?;
    crate::private_file::write_private_atomic(path, serialized.as_bytes())?;
    Ok(())
}
