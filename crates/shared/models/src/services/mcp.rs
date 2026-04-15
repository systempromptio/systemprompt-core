use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerSummary {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    pub enabled: bool,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_debug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_release: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}
