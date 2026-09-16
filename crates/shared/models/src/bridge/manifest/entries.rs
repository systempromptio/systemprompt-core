//! Catalogue entries carried inside a [`super::SignedManifest`]: plugins,
//! skills, rules, agents, hooks and library artifacts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::bridge::ids::{
    LibraryArtifactId, PluginId, RuleId, RuleName, Sha256Digest, SkillId, SkillName,
};
use crate::services::hooks::{HookCategory, HookEvent};
use crate::services::plugin::{PluginComponentRef, PluginHooksRef};
use systemprompt_identifiers::{AgentId, AgentName, HookId, ModelId, ProviderId};

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<SkillPublication>,
    pub id: SkillId,
    pub name: SkillName,
    pub description: String,
    pub file_path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sha256: Sha256Digest,
    pub instructions: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginId>,
}

/// Exact signed publication identity shared by every host projection. This is
/// distribution evidence, not proof that a host installed or invoked the skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPublication {
    pub publication_id: systemprompt_identifiers::PublicationId,
    pub resource_id: systemprompt_identifiers::ManagedResourceId,
    pub revision_id: systemprompt_identifiers::ResourceRevisionId,
    pub generation: i64,
    pub bundle_digest: Sha256Digest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEntry {
    pub id: RuleId,
    pub name: RuleName,
    pub description: String,
    pub file_path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sha256: Sha256Digest,
    pub instructions: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginId>,
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
    pub provider: Option<ProviderId>,
    #[serde(default)]
    pub model: Option<ModelId>,
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
