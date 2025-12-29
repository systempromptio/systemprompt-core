//! Service status extension parameters
//! These are agent-specific types for service orchestration

use serde::{Deserialize, Serialize};

/// Parameters for the service status extension in agent cards
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusParams {
    pub status: String,
    #[serde(default)]
    pub default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}
