//! MCP command output types

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
pub struct McpStatusOutput {
    pub servers: Vec<McpStatusEntry>,
    pub summary: McpStatusSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpStatusEntry {
    pub name: String,
    pub port: u16,
    pub enabled: bool,
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub binary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_binary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_binary: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct McpStatusSummary {
    pub total: usize,
    pub enabled: usize,
    pub running: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpLogsOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    pub logs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub log_files: Vec<String>,
}
