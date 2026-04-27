use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    pub manifest_version: String,
    pub issued_at: String,
    #[serde(default)]
    pub not_before: Option<String>,
    pub user_id: String,
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub user: Option<UserInfo>,
    pub plugins: Vec<PluginEntry>,
    #[serde(default)]
    pub skills: Vec<SkillEntry>,
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub revocations: Vec<String>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub version: String,
    pub sha256: String,
    pub files: Vec<PluginFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file_path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sha256: String,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: String,
    pub name: String,
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
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_policy: Option<BTreeMap<String, String>>,
}

impl SignedManifest {
    pub fn verify(&self, pubkey_b64: &str) -> Result<(), String> {
        let pubkey_bytes = base64::engine::general_purpose::STANDARD
            .decode(pubkey_b64.trim())
            .map_err(|e| format!("pubkey base64 decode: {e}"))?;
        if pubkey_bytes.len() != 32 {
            return Err(format!(
                "pubkey must be 32 bytes (ed25519), got {}",
                pubkey_bytes.len()
            ));
        }
        let arr: [u8; 32] = pubkey_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "pubkey length mismatch".to_string())?;
        let key = VerifyingKey::from_bytes(&arr).map_err(|e| format!("pubkey parse: {e}"))?;

        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(self.signature.trim())
            .map_err(|e| format!("signature base64 decode: {e}"))?;
        if sig_bytes.len() != 64 {
            return Err(format!(
                "signature must be 64 bytes (ed25519), got {}",
                sig_bytes.len()
            ));
        }
        let sig_arr: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "signature length mismatch".to_string())?;
        let signature = Signature::from_bytes(&sig_arr);

        let payload = canonical_payload(self)?;
        key.verify(payload.as_bytes(), &signature)
            .map_err(|e| format!("signature verification failed: {e}"))
    }
}

#[derive(Serialize)]
struct CanonicalView<'a> {
    manifest_version: &'a str,
    issued_at: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    not_before: Option<&'a str>,
    user_id: &'a str,
    tenant_id: Option<&'a str>,
    user: Option<&'a UserInfo>,
    plugins: &'a [PluginEntry],
    skills: &'a [SkillEntry],
    agents: &'a [AgentEntry],
    managed_mcp_servers: &'a [ManagedMcpServer],
    revocations: &'a [String],
}

pub fn canonical_payload(m: &SignedManifest) -> Result<String, String> {
    let view = CanonicalView {
        manifest_version: &m.manifest_version,
        issued_at: &m.issued_at,
        not_before: m.not_before.as_deref(),
        user_id: &m.user_id,
        tenant_id: m.tenant_id.as_deref(),
        user: m.user.as_ref(),
        plugins: &m.plugins,
        skills: &m.skills,
        agents: &m.agents,
        managed_mcp_servers: &m.managed_mcp_servers,
        revocations: &m.revocations,
    };
    serde_jcs::to_string(&view).map_err(|e| format!("canonical serialize: {e}"))
}
