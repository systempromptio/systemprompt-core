use crate::gateway::manifest_version::ManifestVersion;
use crate::ids::{
    ManagedMcpServerName, ManifestSignature, PluginId, Sha256Digest, SkillId, SkillName, ToolName,
    ToolPolicy,
};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    #[error("canonical serialize: {0}")]
    CanonicalSerialize(serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    pub manifest_version: ManifestVersion,
    pub issued_at: String,
    pub not_before: String,
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    #[serde(default)]
    pub user: Option<UserInfo>,
    pub plugins: Vec<PluginEntry>,
    #[serde(default)]
    pub skills: Vec<SkillEntry>,
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub revocations: Vec<String>,
    pub signature: ManifestSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: UserId,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: PluginId,
    pub version: String,
    pub sha256: Sha256Digest,
    pub files: Vec<PluginFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFile {
    pub path: String,
    pub sha256: Sha256Digest,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: SkillId,
    pub name: SkillName,
    pub description: String,
    pub file_path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sha256: Sha256Digest,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: AgentId,
    pub name: AgentName,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub endpoint: String,
    pub enabled: bool,
    pub is_default: bool,
    pub is_primary: bool,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedMcpServer {
    pub name: ManagedMcpServerName,
    pub url: ValidatedUrl,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_policy: Option<BTreeMap<ToolName, ToolPolicy>>,
}

impl SignedManifest {
    pub fn verify(&self, pubkey_b64: &str) -> Result<(), ManifestError> {
        let pubkey_bytes = base64::engine::general_purpose::STANDARD
            .decode(pubkey_b64.trim())
            .map_err(ManifestError::PubkeyBase64)?;
        if pubkey_bytes.len() != 32 {
            return Err(ManifestError::PubkeyLength(pubkey_bytes.len()));
        }
        let arr: [u8; 32] = pubkey_bytes
            .as_slice()
            .try_into()
            .map_err(|_| ManifestError::PubkeyLengthMismatch)?;
        let key = VerifyingKey::from_bytes(&arr).map_err(ManifestError::PubkeyParse)?;

        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(self.signature.as_str().trim())
            .map_err(ManifestError::SignatureBase64)?;
        if sig_bytes.len() != 64 {
            return Err(ManifestError::SignatureLength(sig_bytes.len()));
        }
        let sig_arr: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| ManifestError::SignatureLengthMismatch)?;
        let signature = Signature::from_bytes(&sig_arr);

        let payload = canonical_payload(self)?;
        key.verify(payload.as_bytes(), &signature)
            .map_err(ManifestError::Verify)
    }
}

impl SignedManifest {
    pub fn builder(
        manifest_version: ManifestVersion,
        issued_at: impl Into<String>,
        not_before: impl Into<String>,
        user_id: impl Into<UserId>,
        signature: impl Into<ManifestSignature>,
    ) -> SignedManifestBuilder {
        SignedManifestBuilder {
            manifest_version,
            issued_at: issued_at.into(),
            not_before: not_before.into(),
            user_id: user_id.into(),
            signature: signature.into(),
            tenant_id: None,
            user: None,
            plugins: Vec::new(),
            skills: Vec::new(),
            agents: Vec::new(),
            managed_mcp_servers: Vec::new(),
            revocations: Vec::new(),
        }
    }
}

pub struct SignedManifestBuilder {
    manifest_version: ManifestVersion,
    issued_at: String,
    not_before: String,
    user_id: UserId,
    signature: ManifestSignature,
    tenant_id: Option<TenantId>,
    user: Option<UserInfo>,
    plugins: Vec<PluginEntry>,
    skills: Vec<SkillEntry>,
    agents: Vec<AgentEntry>,
    managed_mcp_servers: Vec<ManagedMcpServer>,
    revocations: Vec<String>,
}

impl SignedManifestBuilder {
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
    pub fn build(self) -> SignedManifest {
        SignedManifest {
            manifest_version: self.manifest_version,
            issued_at: self.issued_at,
            not_before: self.not_before,
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            user: self.user,
            plugins: self.plugins,
            skills: self.skills,
            agents: self.agents,
            managed_mcp_servers: self.managed_mcp_servers,
            revocations: self.revocations,
            signature: self.signature,
        }
    }
}

#[derive(Serialize)]
struct CanonicalView<'a> {
    manifest_version: &'a ManifestVersion,
    issued_at: &'a str,
    not_before: &'a str,
    user_id: &'a UserId,
    tenant_id: Option<&'a TenantId>,
    user: Option<&'a UserInfo>,
    plugins: &'a [PluginEntry],
    skills: &'a [SkillEntry],
    agents: &'a [AgentEntry],
    managed_mcp_servers: &'a [ManagedMcpServer],
    revocations: &'a [String],
}

pub fn canonical_payload(m: &SignedManifest) -> Result<String, ManifestError> {
    let view = CanonicalView {
        manifest_version: &m.manifest_version,
        issued_at: &m.issued_at,
        not_before: &m.not_before,
        user_id: &m.user_id,
        tenant_id: m.tenant_id.as_ref(),
        user: m.user.as_ref(),
        plugins: &m.plugins,
        skills: &m.skills,
        agents: &m.agents,
        managed_mcp_servers: &m.managed_mcp_servers,
        revocations: &m.revocations,
    };
    serde_jcs::to_string(&view).map_err(ManifestError::CanonicalSerialize)
}
