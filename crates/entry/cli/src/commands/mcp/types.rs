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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_binary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_binary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpValidateOutput {
    pub server: String,
    pub valid: bool,
    pub health_status: String,
    pub validation_type: String,
    pub tools_count: usize,
    pub latency_ms: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<McpServerInfo>,
    pub issues: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpBatchValidateOutput {
    pub results: Vec<McpValidateOutput>,
    pub summary: McpValidateSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpValidateSummary {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub healthy: usize,
    pub unhealthy: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpPackagesOutput {
    pub packages: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_packages: Option<String>,
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
    pub source: String,
    pub logs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub log_files: Vec<String>,
}
