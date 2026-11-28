//! The two shapes the scripted instance serves that the harness's projection
//! reads differently, corrected on the way through.
//!
//! The instance's manifest names the protocol tags (`anthropic`,
//! `openai-chat`) under `host_model_protocols`, where core's gateway writes
//! client-family tags (`claude`, `openai`) and the projection admits only
//! those; and its profile lists no provider, so every model is routed to the
//! messages protocol. The relay rewrites the manifest's families, signs it
//! again with a key of its own (served as the instance's key, so `login` pins
//! it), and adds one configured provider to the profile serving every model to
//! the scenario's family. The instance's minimal manifest names no plugin, so
//! no governance plugin either, and the harness asks for a verdict only through
//! one; an empty manifest's plugin list gains one plugin, with no files, that
//! owns the governance hooks. Nothing else changes.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::{Value, json};

const GOVERNANCE_PLUGIN: &str = "governance";
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub(super) struct RelayKey(SigningKey);

impl std::fmt::Debug for RelayKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayKey").finish_non_exhaustive()
    }
}

fn family(tag: &str) -> &str {
    match tag {
        "anthropic" => "claude",
        "openai-chat" | "openai-responses" => "openai",
        other => other,
    }
}

impl RelayKey {
    pub(super) fn generate() -> Self {
        Self(SigningKey::from_bytes(&rand::random::<[u8; 32]>()))
    }

    pub(super) fn pubkey(&self) -> Value {
        json!({ "pubkey": STANDARD.encode(self.0.verifying_key().to_bytes()) })
    }

    /// The envelope with its families corrected and signed with this key.
    pub(super) fn resign(&self, envelope: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut envelope: Value = serde_json::from_slice(envelope)?;
        let payload = envelope
            .get("payload")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut manifest: Value = serde_json::from_str(payload)?;
        if let Some(hosts) = manifest
            .get_mut("host_model_protocols")
            .and_then(Value::as_object_mut)
        {
            for tags in hosts.values_mut() {
                if let Some(list) = tags.as_array_mut() {
                    for tag in list.iter_mut() {
                        if let Some(text) = tag.as_str() {
                            *tag = Value::String(family(text).to_owned());
                        }
                    }
                }
            }
        }
        let plugins = manifest.get_mut("plugins").and_then(Value::as_array_mut);
        if let Some(plugins) = plugins.filter(|plugins| plugins.is_empty()) {
            plugins.push(json!({
                "id": GOVERNANCE_PLUGIN,
                "version": "1.0.0",
                "sha256": EMPTY_SHA256,
                "files": [],
                "hooks": { "governance": true },
            }));
        }
        let payload = serde_jcs::to_string(&manifest)?;
        let signature = STANDARD.encode(self.0.sign(payload.as_bytes()).to_bytes());
        envelope["payload"] = Value::String(payload);
        envelope["signature"] = Value::String(signature);
        Ok(serde_json::to_vec(&envelope)?)
    }
}

/// The profile with one configured provider serving every listed model to
/// `family`.
pub(super) fn with_provider(profile: &[u8], family: &str) -> anyhow::Result<Vec<u8>> {
    let mut profile: Value = serde_json::from_slice(profile)?;
    let models = profile.get("models").cloned().unwrap_or_else(|| json!([]));
    profile["providers"] = json!([{
        "name": "scripted",
        "credential": "platform",
        "clients": [family],
        "configured": true,
        "models": models,
    }]);
    Ok(serde_json::to_vec(&profile)?)
}
