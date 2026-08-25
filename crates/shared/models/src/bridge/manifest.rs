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
use systemprompt_identifiers::{
    AgentId, AgentName, HookId, McpServerId, TenantId, UserId, ValidatedUrl,
};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

// Why: not tied to the release version, and never swept by a version-bump
// script. Raising it strands every client below it until they update, so it
// moves only when the gateway makes a change an older bridge cannot handle.
pub const MIN_BRIDGE_VERSION: &str = "0.28.0";

#[must_use]
pub fn bridge_version_is_supported(reported: &str, floor: &str) -> bool {
    match (
        semver::Version::parse(reported),
        semver::Version::parse(floor),
    ) {
        (Ok(reported), Ok(floor)) => reported >= floor,
        // Why: an unparseable version is almost always a local dev build;
        // refusing those would make the gateway untestable against a work tree.
        _ => true,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifestEnvelope {
    pub payload: String,
    pub signature: ManifestSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    #[serde(default)]
    pub min_schema_version: u32,
    #[serde(default)]
    pub min_bridge_version: Option<String>,
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
    #[serde(default)]
    pub host_model_protocols: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactEntry>,
    #[serde(default)]
    pub allow_claude_ai_connectors: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
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

// Why: field names and casing must track Cowork's native `create_artifact`
// input, so a consumer can read a bundle's `artifacts/<id>.json` and the
// bridge's staged library records with one parser.
#[derive(Debug, Serialize)]
pub struct CoworkLibraryArtifactRecord<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub version: &'a str,
    pub content: &'a str,
    #[serde(rename = "isStarred")]
    pub is_starred: bool,
    #[serde(rename = "mcpTools")]
    pub mcp_tools: &'a [String],
}

impl<'a> From<&'a ArtifactEntry> for CoworkLibraryArtifactRecord<'a> {
    fn from(a: &'a ArtifactEntry) -> Self {
        Self {
            id: a.id.as_str(),
            name: &a.name,
            description: &a.description,
            version: &a.version,
            content: &a.content,
            is_starred: a.starred,
            mcp_tools: &a.mcp_tools,
        }
    }
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
#[serde(from = "ManagedMcpServerWire")]
pub struct ManagedMcpServer {
    pub id: McpServerId,
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

// Why: manifests signed before the `id` field existed carry only `name`, so
// deserialization derives an absent id from it.
#[derive(Deserialize)]
struct ManagedMcpServerWire {
    #[serde(default)]
    id: Option<McpServerId>,
    name: ManagedMcpServerName,
    url: ValidatedUrl,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    headers: Option<BTreeMap<String, String>>,
    #[serde(default)]
    oauth: Option<bool>,
    #[serde(default)]
    tool_policy: Option<BTreeMap<ToolName, ToolPolicy>>,
}

impl From<ManagedMcpServerWire> for ManagedMcpServer {
    fn from(wire: ManagedMcpServerWire) -> Self {
        let id = wire
            .id
            .unwrap_or_else(|| McpServerId::new(wire.name.as_str()));
        Self {
            id,
            name: wire.name,
            url: wire.url,
            transport: wire.transport,
            headers: wire.headers,
            oauth: wire.oauth,
            tool_policy: wire.tool_policy,
        }
    }
}
