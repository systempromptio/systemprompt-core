//! Global MCP deployment settings shared by every server in a
//! [`super::deployment::DeploymentConfig`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub auto_build: bool,
    pub build_timeout: u64,
    pub health_check_timeout: u64,
    #[serde(default = "default_base_port")]
    pub base_port: u16,
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
}

const fn default_base_port() -> u16 {
    5000
}

fn default_working_dir() -> String {
    "/app".to_owned()
}
