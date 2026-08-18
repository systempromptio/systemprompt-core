//! Signed manifest wire format.
//!
//! `GET /v1/bridge/manifest` returns a [`SignedManifestEnvelope`]: the
//! JCS-canonical serialization of a [`SignedManifest`] carried verbatim as
//! `payload`, plus a detached ed25519 signature over those exact bytes. The
//! bridge verifies the signature against the raw `payload` string *before*
//! deserialising it, so fields added to [`SignedManifest`] in newer gateways
//! never invalidate the signature on older bridges — unknown fields are
//! simply ignored at parse time. Semantic breaks that an older bridge cannot
//! safely ignore are declared by raising `min_schema_version` above
//! [`MANIFEST_SCHEMA_VERSION`] of the consuming bridge, which then refuses
//! with an upgrade message instead of a signature error.
//!
//! Signing, signature verification, and manifest construction live in
//! the bridge crate (`bin/bridge/src/gateway/manifest.rs`) alongside
//! the gateway client. Those layers pull in `ed25519-dalek` and
//! `serde_jcs` which are not appropriate dependencies for this
//! foundation crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use crate::bridge::ids::ManifestSignature;
use crate::bridge::ids::{
    LibraryArtifactId, ManagedMcpServerName, PluginId, Sha256Digest, SkillId, SkillName, ToolName,
    ToolPolicy,
};
use crate::bridge::manifest_version::ManifestVersion;
use crate::services::hooks::{HookCategory, HookEvent};
use crate::services::plugin::{PluginComponentRef, PluginHooksRef};
use systemprompt_identifiers::{AgentId, AgentName, HookId, TenantId, UserId, ValidatedUrl};

/// Schema level this build of the codebase emits and understands.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifestEnvelope {
    /// JCS-canonical [`SignedManifest`] JSON, signed byte-for-byte. Consumers
    /// must verify the signature over this exact string before parsing it.
    pub payload: String,
    /// Detached ed25519 signature over `payload`; the empty string on
    /// unsigned installations.
    pub signature: ManifestSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    /// Oldest schema level that can safely consume this manifest. Additive
    /// fields leave it unchanged; only semantic breaks raise it.
    #[serde(default)]
    pub min_schema_version: u32,
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
    /// Optional per-host wire-protocol filter, keyed by host id. A present
    /// entry overrides the host's built-in default `accepted_protocols`; an
    /// empty value means "all models" (no restriction). An absent entry leaves
    /// the host on its default.
    #[serde(default)]
    pub host_model_protocols: BTreeMap<String, Vec<String>>,
    /// Cowork global-library HTML documents — distinct from the in-chat MCP
    /// artifacts in [`crate::artifacts`].
    #[serde(default)]
    pub artifacts: Vec<ArtifactEntry>,
    /// Instructs the bridge's Claude Code managed-MCP policy to emit
    /// `allowAllClaudeAiMcps`, re-allowing claude.ai first-party connectors
    /// that `managed-mcp.json` would otherwise suppress.
    #[serde(default)]
    pub allow_claude_ai_connectors: bool,
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
    #[serde(default)]
    pub hooks: PluginHooksRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFile {
    pub path: String,
    pub sha256: Sha256Digest,
    pub size: u64,
}

/// A Cowork-native library document (raw HTML in the desktop app's Artifacts
/// library) — not one of the in-chat MCP artifacts in [`crate::artifacts`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub id: LibraryArtifactId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub mcp_tools: Vec<String>,
    pub content: String,
    pub starred: bool,
    pub sha256: Sha256Digest,
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
    pub mcp_servers: PluginComponentRef,
    #[serde(default)]
    pub skills: PluginComponentRef,
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
