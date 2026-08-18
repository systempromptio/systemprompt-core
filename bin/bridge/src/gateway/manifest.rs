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
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

pub use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, MANIFEST_SCHEMA_VERSION, ManagedMcpServer, PluginEntry,
    PluginFile, SignedManifest, SignedManifestEnvelope, SkillEntry, UserInfo,
};
pub use systemprompt_models::bridge::manifest_version::ManifestVersion;
pub use systemprompt_models::services::PluginComponentRef;

pub use systemprompt_identifiers::{AgentId, AgentName, TenantId, UserId, ValidatedUrl};

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

    key.verify(envelope.payload.as_bytes(), &signature)
        .map_err(ManifestError::Verify)
}

pub fn decode_payload(envelope: &SignedManifestEnvelope) -> Result<SignedManifest, ManifestError> {
    let manifest: SignedManifest =
        serde_json::from_str(&envelope.payload).map_err(ManifestError::PayloadParse)?;
    if manifest.min_schema_version > MANIFEST_SCHEMA_VERSION {
        return Err(ManifestError::SchemaTooNew {
            required: manifest.min_schema_version,
            supported: MANIFEST_SCHEMA_VERSION,
        });
    }
    Ok(manifest)
}

#[derive(Debug)]
pub struct SignedManifestBuilder {
    manifest_version: ManifestVersion,
    issued_at: String,
    not_before: String,
    user_id: UserId,
    tenant_id: Option<TenantId>,
    user: Option<UserInfo>,
    plugins: Vec<PluginEntry>,
    skills: Vec<SkillEntry>,
    agents: Vec<AgentEntry>,
    hooks: Vec<HookEntry>,
    managed_mcp_servers: Vec<ManagedMcpServer>,
    revocations: Vec<String>,
    enabled_hosts: Vec<String>,
    host_model_protocols: std::collections::BTreeMap<String, Vec<String>>,
    artifacts: Vec<ArtifactEntry>,
    allow_claude_ai_connectors: bool,
}

impl SignedManifestBuilder {
    #[must_use]
    pub fn new(
        manifest_version: ManifestVersion,
        issued_at: impl Into<String>,
        not_before: impl Into<String>,
        user_id: impl Into<UserId>,
    ) -> Self {
        Self {
            manifest_version,
            issued_at: issued_at.into(),
            not_before: not_before.into(),
            user_id: user_id.into(),
            tenant_id: None,
            user: None,
            plugins: Vec::new(),
            skills: Vec::new(),
            agents: Vec::new(),
            hooks: Vec::new(),
            managed_mcp_servers: Vec::new(),
            revocations: Vec::new(),
            enabled_hosts: Vec::new(),
            host_model_protocols: std::collections::BTreeMap::new(),
            artifacts: Vec::new(),
            allow_claude_ai_connectors: false,
        }
    }

    #[must_use]
    pub const fn with_allow_claude_ai_connectors(mut self, allow: bool) -> Self {
        self.allow_claude_ai_connectors = allow;
        self
    }

    #[must_use]
    pub fn with_enabled_hosts(mut self, hosts: Vec<String>) -> Self {
        self.enabled_hosts = hosts;
        self
    }

    #[must_use]
    pub fn with_host_model_protocols(
        mut self,
        protocols: std::collections::BTreeMap<String, Vec<String>>,
    ) -> Self {
        self.host_model_protocols = protocols;
        self
    }

    #[must_use]
    pub fn with_tenant_id(mut self, tenant_id: impl Into<TenantId>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    #[must_use]
    pub fn with_user(mut self, user: UserInfo) -> Self {
        self.user = Some(user);
        self
    }

    #[must_use]
    pub fn with_plugins(mut self, plugins: Vec<PluginEntry>) -> Self {
        self.plugins = plugins;
        self
    }

    #[must_use]
    pub fn with_skills(mut self, skills: Vec<SkillEntry>) -> Self {
        self.skills = skills;
        self
    }

    #[must_use]
    pub fn with_agents(mut self, agents: Vec<AgentEntry>) -> Self {
        self.agents = agents;
        self
    }

    #[must_use]
    pub fn with_hooks(mut self, hooks: Vec<HookEntry>) -> Self {
        self.hooks = hooks;
        self
    }

    #[must_use]
    pub fn with_managed_mcp_servers(mut self, servers: Vec<ManagedMcpServer>) -> Self {
        self.managed_mcp_servers = servers;
        self
    }

    #[must_use]
    pub fn with_revocations(mut self, revocations: Vec<String>) -> Self {
        self.revocations = revocations;
        self
    }

    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Vec<ArtifactEntry>) -> Self {
        self.artifacts = artifacts;
        self
    }

    #[must_use]
    pub fn build(self) -> SignedManifest {
        SignedManifest {
            min_schema_version: MANIFEST_SCHEMA_VERSION,
            manifest_version: self.manifest_version,
            issued_at: self.issued_at,
            not_before: self.not_before,
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            user: self.user,
            plugins: self.plugins,
            skills: self.skills,
            agents: self.agents,
            hooks: self.hooks,
            managed_mcp_servers: self.managed_mcp_servers,
            revocations: self.revocations,
            enabled_hosts: self.enabled_hosts,
            host_model_protocols: self.host_model_protocols,
            artifacts: self.artifacts,
            allow_claude_ai_connectors: self.allow_claude_ai_connectors,
        }
    }
}
