//! The entries of a manifest: skills, rules, agents, managed MCP servers and
//! plugin bundles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Publication, Sha256Digest, ToolPolicy};
use crate::ids::{AgentId, MarketplaceRuleId, PluginId, SkillId};

/// Where a schema-2 skill's body lives inside a plugin bundle.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSource {
    #[serde(default)]
    pub plugin: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

/// A skill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: SkillId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub source: Option<SkillSource>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub sha256: Option<Sha256Digest>,
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default)]
    pub plugins: Vec<PluginId>,
    #[serde(default)]
    pub tags: Vec<String>,
    // JSON: the author's SKILL.md front matter, passed through to the rendered file.
    #[serde(default)]
    pub frontmatter: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub publication: Option<Publication>,
}

/// A rule: governance instructions for coding agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEntry {
    pub id: MarketplaceRuleId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub sha256: Option<Sha256Digest>,
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default)]
    pub plugins: Vec<PluginId>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// An agent's selection of MCP servers or skills.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// An agent. Agents are projected from plugin bundles, not from these entries;
/// they are read for counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: AgentId,
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub mcp_servers: Option<Selection>,
    #[serde(default)]
    pub skills: Option<Selection>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub version: Option<String>,
}

/// A managed remote MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedMcpServer {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub oauth: Option<bool>,
    #[serde(default)]
    pub tool_policy: BTreeMap<String, ToolPolicy>,
}

impl ManagedMcpServer {
    pub const TOOL_POLICY_WILDCARD: &'static str = "*";

    #[must_use]
    pub fn id(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.name)
    }

    #[must_use]
    pub fn policy_for_tool(&self, tool: &str) -> Option<ToolPolicy> {
        self.tool_policy
            .get(tool)
            .or_else(|| self.tool_policy.get(Self::TOOL_POLICY_WILDCARD))
            .copied()
    }
}

/// Which hook families a plugin owns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginHooks {
    #[serde(default)]
    pub governance: bool,
    #[serde(default)]
    pub judge: bool,
    #[serde(default)]
    pub comms: bool,
}

/// One file of a plugin bundle, fetched from `paths::PLUGIN_FILES` and checked
/// against `sha256` before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginFile {
    pub path: String,
    pub sha256: Sha256Digest,
    #[serde(default)]
    pub size: Option<u64>,
}

/// A plugin bundle the user is granted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: PluginId,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub hooks: PluginHooks,
    #[serde(default)]
    pub sha256: Option<Sha256Digest>,
    #[serde(default)]
    pub files: Vec<PluginFile>,
    #[serde(default)]
    pub publication: Option<Publication>,
}
