//! ed25519 verification and decoding of signed gateway manifest envelopes.
//!
//! The gateway serves a [`SignedManifestEnvelope`] whose `payload` is the
//! exact canonical string it signed. Verification runs over those raw bytes
//! before any deserialisation, so manifest fields this bridge does not know
//! about can never break the signature — schema compatibility is negotiated
//! separately via `min_schema_version`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};

pub use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, MANIFEST_INLINE_SCHEMA_VERSION, MANIFEST_SCHEMA_HEADER,
    MANIFEST_SCHEMA_VERSION, ManagedMcpServer, ManifestClaudeCode, ManifestMarketplace,
    PluginEntry, PluginFile, RuleEntry, SignedManifest, SignedManifestEnvelope, SkillEntry,
    SkillSource, UserInfo, bridge_version_is_supported,
};
pub use systemprompt_models::bridge::manifest_version::ManifestVersion;
pub use systemprompt_models::bridge::update_policy::AutoUpdatePolicy;
pub use systemprompt_models::plugin::PluginComponentRef;

pub use systemprompt_identifiers::{AgentId, AgentName, ApiKeyId, TenantId, UserId, ValidatedUrl};

mod builder;

pub use super::manifest_hosts::{
    enables_host, skill_targets_host, skill_targets_product, unknown_skill_hosts,
};
pub use builder::SignedManifestBuilder;

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("pubkey base64 decode: {0}")]
    PubkeyBase64(base64::DecodeError),
    #[error("pubkey must be 32 bytes (ed25519), got {0}")]
    PubkeyLength(usize),
    #[error("pubkey length mismatch")]
    PubkeyLengthMismatch,
    #[error("pubkey parse: {0}")]
    PubkeyParse(ed25519_dalek::SignatureError),
    #[error("signature base64 decode: {0}")]
    SignatureBase64(base64::DecodeError),
    #[error("signature must be 64 bytes (ed25519), got {0}")]
    SignatureLength(usize),
    #[error("signature length mismatch")]
    SignatureLengthMismatch,
    #[error("signature verification failed: {0}")]
    Verify(ed25519_dalek::SignatureError),
    #[error("manifest payload parse: {0}")]
    PayloadParse(serde_json::Error),
    #[error(
        "manifest requires schema {required} but this bridge supports up to {supported}; \
         upgrade the bridge"
    )]
    SchemaTooNew { required: u32, supported: u32 },
    #[error("bridge {local} is older than the {required} this gateway requires")]
    BridgeTooOld { local: String, required: String },
}

pub fn verify_envelope(
    envelope: &SignedManifestEnvelope,
    pubkey_b64: &str,
) -> Result<(), ManifestError> {
    let pubkey_bytes = base64::engine::general_purpose::STANDARD
        .decode(pubkey_b64.trim())
        .map_err(ManifestError::PubkeyBase64)?;
    if pubkey_bytes.len() != 32 {
        return Err(ManifestError::PubkeyLength(pubkey_bytes.len()));
    }
    let arr: [u8; 32] = pubkey_bytes
        .as_slice()
        .try_into()
        .map_err(|_len| ManifestError::PubkeyLengthMismatch)?;
    let key = VerifyingKey::from_bytes(&arr).map_err(ManifestError::PubkeyParse)?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(envelope.signature.as_str().trim())
        .map_err(ManifestError::SignatureBase64)?;
    if sig_bytes.len() != 64 {
        return Err(ManifestError::SignatureLength(sig_bytes.len()));
    }
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_len| ManifestError::SignatureLengthMismatch)?;
    let signature = Signature::from_bytes(&sig_arr);

    key.verify_strict(envelope.payload.as_bytes(), &signature)
        .map_err(ManifestError::Verify)
}

pub fn decode_payload(envelope: &SignedManifestEnvelope) -> Result<SignedManifest, ManifestError> {
    #[derive(serde::Deserialize)]
    struct VersionProbe {
        #[serde(default)]
        min_schema_version: Option<u32>,
        #[serde(default)]
        min_bridge_version: Option<String>,
    }
    let local = crate::brand::COMPAT_VERSION;
    if let Ok(probe) = serde_json::from_str::<VersionProbe>(&envelope.payload) {
        if let Some(required) = probe.min_schema_version
            && required > MANIFEST_SCHEMA_VERSION
        {
            return Err(ManifestError::SchemaTooNew {
                required,
                supported: MANIFEST_SCHEMA_VERSION,
            });
        }
        if let Some(required) = probe.min_bridge_version.as_deref()
            && !semver::Version::parse(required)
                .is_ok_and(|floor| bridge_version_is_supported(local, &floor))
        {
            return Err(ManifestError::BridgeTooOld {
                local: local.to_owned(),
                required: required.to_owned(),
            });
        }
    }
    serde_json::from_str(&envelope.payload).map_err(ManifestError::PayloadParse)
}
