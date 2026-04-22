use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    pub manifest_version: String,
    pub issued_at: String,
    pub user_id: String,
    pub tenant_id: Option<String>,
    pub plugins: Vec<PluginEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub revocations: Vec<String>,
    pub signature: String,
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
        "plugins": m.plugins,
        "managed_mcp_servers": m.managed_mcp_servers,
        "revocations": m.revocations,
    });
    serde_json::to_string(&view).map_err(|e| format!("canonical serialize: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_payload_excludes_signature() {
        let m = SignedManifest {
            manifest_version: "v1".into(),
            issued_at: "2026-04-22T00:00:00Z".into(),
            user_id: "u1".into(),
            tenant_id: None,
            plugins: vec![],
            managed_mcp_servers: vec![],
            revocations: vec![],
            signature: "SHOULD-NOT-APPEAR".into(),
        };
        let payload = canonical_payload(&m).unwrap();
        assert!(!payload.contains("SHOULD-NOT-APPEAR"));
        assert!(payload.contains("v1"));
    }
}
