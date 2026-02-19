use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::ai::ToolModelConfig;
use super::IncludableString;

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

pub fn strip_frontmatter(content: &str) -> String {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() >= 3 {
        parts[2].trim().to_string()
    } else {
        content.to_string()
    }
}
