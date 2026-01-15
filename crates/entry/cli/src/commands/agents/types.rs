use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_models::a2a::Task;

use crate::commands::mcp::types::McpToolEntry;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentListOutput {
    pub agents: Vec<AgentSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentSummary {
    pub name: String,
    pub display_name: String,
    pub port: u16,
    pub enabled: bool,
    pub is_primary: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentDetailOutput {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub port: u16,
    pub endpoint: String,
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub mcp_servers: Vec<String>,
    pub skills_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationOutput {
    pub valid: bool,
    pub agents_checked: usize,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationIssue {
    pub agent: String,
    pub severity: ValidationSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentStatusOutput {
    pub agents: Vec<AgentStatusRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentStatusRow {
    pub name: String,
    pub enabled: bool,
    pub is_running: bool,
    pub pid: Option<u32>,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentCreateOutput {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentEditOutput {
    pub name: String,
    pub message: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentDeleteOutput {
    pub deleted: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpListOutput {
    pub servers: Vec<McpServerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerSummary {
    pub name: String,
    pub port: u16,
    pub enabled: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpValidateOutput {
    pub server: String,
    pub valid: bool,
    pub tools_count: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpPackagesOutput {
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentLogsOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub source: String,
    pub logs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub log_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegistryOutput {
    pub gateway_url: String,
    pub agents_count: usize,
    pub agents: Vec<RegistryAgentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegistryAgentInfo {
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub status: String,
    pub streaming: bool,
    pub skills_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageOutput {
    pub agent: String,
    pub task: Task,
    pub message_sent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentToolsOutput {
    pub agent: String,
    pub tools: Vec<McpToolEntry>,
    pub summary: AgentToolsSummary,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unavailable_servers: Vec<UnavailableServer>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct AgentToolsSummary {
    pub total_tools: usize,
    pub configured_servers: usize,
    pub available_servers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnavailableServer {
    pub name: String,
    pub reason: String,
}
