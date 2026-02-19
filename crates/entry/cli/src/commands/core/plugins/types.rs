use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_models::{ComponentFilter, ComponentSource};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginListOutput {
    pub plugins: Vec<PluginSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub skill_count: usize,
    pub agent_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginDetailOutput {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub skills: PluginComponentDetail,
    pub agents: PluginComponentDetail,
    pub mcp_servers: Vec<String>,
    pub hooks_count: usize,
    pub scripts: Vec<String>,
    pub keywords: Vec<String>,
    pub category: String,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginComponentDetail {
    pub source: ComponentSource,
    pub filter: Option<ComponentFilter>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginValidateOutput {
    pub plugin_id: String,
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginValidateAllOutput {
    pub results: Vec<PluginValidateOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginGenerateOutput {
    pub plugin_id: String,
    pub files_generated: Vec<String>,
    pub marketplace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginGenerateAllOutput {
    pub results: Vec<PluginGenerateOutput>,
    pub install_command: Option<String>,
}
