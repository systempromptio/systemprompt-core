//! Serializable output payloads for the `admin agents` command group.
//!
//! Each command renders through one of these types — listings, per-agent
//! detail and status, create/edit/delete results, log views, registry
//! discovery, A2A message output, and MCP tool summaries — so that table, JSON,
//! and YAML rendering share a single typed contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::AgentName;
pub use systemprompt_manifest::services::AgentSummary;
use systemprompt_models::a2a::Task;


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationOutput {
    pub valid: bool,
    pub items_checked: usize,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationIssue {
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentListOutput {
    pub agents: Vec<AgentSummary>,
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
    pub name: AgentName,
    pub message: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentDeleteOutput {
    pub deleted: Vec<AgentName>,
    pub message: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Task>,
    pub message_sent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
}
