use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::PluginId;

use super::capability_types::CapabilitySummary;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CliExtensionInfo {
    pub name: String,
    pub binary: String,
    pub description: String,
    pub commands: Vec<CliCommandInfo>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CliCommandInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CliExtListOutput {
    pub extensions: Vec<CliExtensionInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSource {
    Compiled,
    Manifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionSummary {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub priority: u32,
    pub source: ExtensionSource,
    pub enabled: bool,
    pub capabilities: CapabilitySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionListOutput {
    pub extensions: Vec<ExtensionSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobInfo {
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemaInfo {
    pub table: String,
    pub source: String,
    pub required_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RouteInfo {
    pub base_path: String,
    pub requires_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProviderInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionDetailOutput {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub priority: u32,
    pub source: ExtensionSource,
    pub dependencies: Vec<String>,
    pub config_prefix: Option<String>,
    pub jobs: Vec<JobInfo>,
    pub templates: Vec<TemplateInfo>,
    pub schemas: Vec<SchemaInfo>,
    pub routes: Vec<RouteInfo>,
    pub tools: Vec<ToolInfo>,
    pub roles: Vec<RoleInfo>,
    pub llm_providers: Vec<LlmProviderInfo>,
    pub storage_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionConfigOutput {
    pub extension_id: PluginId,
    pub config_prefix: Option<String>,
    pub config_schema: Option<serde_json::Value>,
    pub has_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionConfigSummary {
    pub extension_id: PluginId,
    pub config_prefix: Option<String>,
    pub has_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionConfigListOutput {
    pub extensions: Vec<ExtensionConfigSummary>,
    pub total: usize,
}
