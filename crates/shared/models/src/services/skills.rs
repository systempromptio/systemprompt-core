use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::ai::ToolModelConfig;
use super::IncludableString;

const fn default_true() -> bool {
    true
}

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
