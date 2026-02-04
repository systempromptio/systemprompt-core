use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopServiceOutput {
    pub api_stopped: bool,
    pub agents_stopped: usize,
    pub mcp_stopped: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopIndividualOutput {
    pub service_type: String,
    pub service_name: String,
    pub stopped: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CleanupOutput {
    pub services_cleaned: usize,
    pub stale_entries_removed: usize,
    pub dry_run: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RestartOutput {
    pub service_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    pub restarted_count: usize,
    pub failed_count: usize,
    pub message: String,
}
