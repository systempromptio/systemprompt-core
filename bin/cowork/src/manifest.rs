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

pub fn canonical_payload(m: &SignedManifest) -> Result<String, String> {
    let view = serde_json::json!({
        "manifest_version": m.manifest_version,
        "issued_at": m.issued_at,
        "user_id": m.user_id,
        "tenant_id": m.tenant_id,
        "user": m.user,
        "plugins": m.plugins,
        "skills": m.skills,
        "agents": m.agents,
        "managed_mcp_servers": m.managed_mcp_servers,
        "revocations": m.revocations,
    });
    serde_jcs::to_string(&view).map_err(|e| format!("canonical serialize: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_payload_excludes_signature() {
        let m = SignedManifest {
            manifest_version: "v1".into(),
            issued_at: "2026-04-22T00:00:00Z".into(),
            not_before: None,
            user_id: "u1".into(),
            tenant_id: None,
            user: None,
            plugins: vec![],
            skills: vec![],
            agents: vec![],
            managed_mcp_servers: vec![],
            revocations: vec![],
            signature: "SHOULD-NOT-APPEAR".into(),
        };
        let payload = canonical_payload(&m).unwrap();
        assert!(!payload.contains("SHOULD-NOT-APPEAR"));
        assert!(payload.contains("v1"));
    }

    #[test]
    fn canonical_payload_includes_user_skills_agents() {
        let m = SignedManifest {
            manifest_version: "v2".into(),
            issued_at: "2026-04-22T00:00:00Z".into(),
            not_before: None,
            user_id: "u1".into(),
            tenant_id: None,
            user: Some(UserInfo {
                id: "u1".into(),
                name: "alice".into(),
                email: "a@e.com".into(),
                display_name: Some("Alice".into()),
                roles: vec!["admin".into()],
            }),
            plugins: vec![],
            skills: vec![SkillEntry {
                id: "s1".into(),
                name: "Skill 1".into(),
                description: "desc".into(),
                file_path: "/skills/s1.md".into(),
                tags: vec![],
                sha256: "abc".into(),
                instructions: "do the thing".into(),
            }],
            agents: vec![AgentEntry {
                id: "a1".into(),
                name: "agent1".into(),
                display_name: "Agent 1".into(),
                description: "d".into(),
                version: "1.0".into(),
                endpoint: "/api/agent1".into(),
                enabled: true,
                is_default: false,
                is_primary: true,
                provider: Some("anthropic".into()),
                model: Some("claude".into()),
                mcp_servers: vec!["github".into()],
                skills: vec!["s1".into()],
                tags: vec![],
                system_prompt: None,
            }],
            managed_mcp_servers: vec![],
            revocations: vec![],
            signature: "x".into(),
        };
        let payload = canonical_payload(&m).unwrap();
        assert!(payload.contains("alice"));
        assert!(payload.contains("Skill 1"));
        assert!(payload.contains("agent1"));
    }
}
