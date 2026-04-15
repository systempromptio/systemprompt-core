use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::IncludableString;
use super::ai::ToolModelConfig;

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
    pub id: String,
    pub name: String,
    pub description: String,

    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub instructions: Option<IncludableString>,

    #[serde(default)]
    pub assigned_agents: Vec<String>,

    #[serde(default)]
    pub mcp_servers: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<ToolModelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskSkillConfig {
    pub id: String,
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
}

impl DiskSkillConfig {
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
    pub skill_id: String,
    pub name: String,
    pub display_name: String,
    pub enabled: bool,
    pub file_path: Option<String>,
    pub tags: Vec<String>,
}

impl From<&DiskSkillConfig> for SkillSummary {
    fn from(config: &DiskSkillConfig) -> Self {
        let file_path = if config.file.is_empty() {
            None
        } else {
            Some(config.file.clone())
        };
        Self {
            skill_id: config.id.clone(),
            name: config.name.clone(),
            display_name: config.name.clone(),
            enabled: config.enabled,
            file_path,
            tags: config.tags.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillDetail {
    pub skill_id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub file_path: Option<String>,
    pub instructions_preview: String,
}

impl From<&DiskSkillConfig> for SkillDetail {
    fn from(config: &DiskSkillConfig) -> Self {
        let file_path = if config.file.is_empty() {
            None
        } else {
            Some(config.file.clone())
        };
        Self {
            skill_id: config.id.clone(),
            name: config.name.clone(),
            display_name: config.name.clone(),
            description: config.description.clone(),
            enabled: config.enabled,
            tags: config.tags.clone(),
            category: config.category.clone(),
            file_path,
            instructions_preview: String::new(),
        }
    }
}

pub fn strip_frontmatter(content: &str) -> String {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() >= 3 {
        parts[2].trim().to_string()
    } else {
        content.to_string()
    }
}
