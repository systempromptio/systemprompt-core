//! Skill configuration and disk-descriptor model.
//!
//! [`SkillsConfig`] is the top-level block (discovery, path, per-skill map);
//! [`SkillConfig`] is the in-profile definition and [`DiskSkillConfig`] the
//! per-skill on-disk descriptor. [`SkillSummary`] and [`SkillDetail`] are the
//! list- and detail-view projections.
//!
//! A descriptor may leave `id` empty: the skill is then named by its
//! directory, which [`DiskSkillConfig::resolved_id`] applies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use systemprompt_identifiers::SkillId;
use systemprompt_identifiers::error::IdValidationError;

use super::IncludableString;
use super::plugin::PluginComponentRef;
use crate::ai::ToolModelConfig;
use crate::bridge::host::{HostKind, UnknownHostKind};

const fn default_true() -> bool {
    true
}

pub const SKILL_CONFIG_FILENAME: &str = "config.yaml";
pub const DEFAULT_SKILL_CONTENT_FILE: &str = "index.md";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub auto_discover: bool,

    #[serde(default)]
    pub skills_path: Option<String>,

    #[serde(default)]
    pub skills: HashMap<String, SkillConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    pub id: SkillId,
    pub name: String,
    pub description: String,

    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub instructions: Option<IncludableString>,

    #[serde(default)]
    pub assigned_agents: PluginComponentRef,

    #[serde(default)]
    pub mcp_servers: PluginComponentRef,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<ToolModelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskSkillConfig {
    #[serde(deserialize_with = "empty_skill_id_as_none")]
    pub id: Option<SkillId>,
    pub name: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default)]
    pub frontmatter: Option<serde_yaml::Mapping>,
}

fn empty_skill_id_as_none<'de, D>(deserializer: D) -> Result<Option<SkillId>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .filter(|raw| !raw.trim().is_empty())
        .map(SkillId::try_new)
        .transpose()
        .map_err(serde::de::Error::custom)
}

impl DiskSkillConfig {
    pub fn resolved_id(&self, dir_name: &str) -> Result<SkillId, IdValidationError> {
        match &self.id {
            Some(id) => Ok(id.clone()),
            None => SkillId::try_new(dir_name),
        }
    }

    pub fn host_kinds(&self) -> Result<Vec<HostKind>, UnknownHostKind> {
        HostKind::parse_list(&self.hosts)
    }

    pub fn content_file(&self) -> &str {
        if self.file.is_empty() {
            DEFAULT_SKILL_CONTENT_FILE
        } else {
            &self.file
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillSummary {
    pub skill_id: SkillId,
    pub name: String,
    pub display_name: String,
    pub enabled: bool,
    pub file_path: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillDetail {
    pub skill_id: SkillId,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub file_path: Option<String>,
    pub instructions_preview: String,
}
