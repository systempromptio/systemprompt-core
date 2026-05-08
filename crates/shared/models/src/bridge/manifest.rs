//! Signed manifest wire format.
//!
//! [`SignedManifest`] is the JSON document the gateway returns from
//! `GET /v1/bridge/manifest` and the bridge consumes to drive its
//! plugin / skill / agent / managed-MCP sync. Every public type in
//! this module is part of that wire contract — the gateway server
//! (in `crates/entry/api`) emits these structs and the bridge
//! deserialises them, so any change here is a wire-format change.
//!
//! Signing, signature verification, and manifest construction live in
//! the bridge crate (`bin/bridge/src/gateway/manifest.rs`) alongside
//! the gateway client. Those layers pull in `ed25519-dalek` and
//! `serde_jcs` which are not appropriate dependencies for this
//! foundation crate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bridge::ids::{
    ManagedMcpServerName, ManifestSignature, PluginId, Sha256Digest, SkillId, SkillName, ToolName,
    ToolPolicy,
};
use crate::bridge::manifest_version::ManifestVersion;
use crate::services::hooks::{HookCategory, HookEvent};
use systemprompt_identifiers::{AgentId, AgentName, HookId, TenantId, UserId, ValidatedUrl};

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
    #[serde(default)]
    pub hooks: Vec<HookEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub revocations: Vec<String>,
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
    /// Detached ed25519 signature of the canonicalised payload (every
    /// field above this one). Always present on the wire even for
    /// unsigned manifests, where it is the empty string.
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
pub struct HookEntry {
    pub id: HookId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub event: HookEvent,
    pub matcher: String,
    pub command: String,
    #[serde(default)]
    pub is_async: bool,
    pub category: HookCategory,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sha256: Sha256Digest,
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
