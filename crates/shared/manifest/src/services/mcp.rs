//! Managed MCP server definitions in services config.
//!
//! [`McpDeploymentProvider`] loads the services manifest an MCP deployment is
//! read from. It is called on concrete implementations, never held as a trait
//! object, so it declares native `async` methods and returns
//! `McpRegistryResult`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_models::errors::McpRegistryResult;

use crate::ServicesConfig;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerSummary {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub server_type: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
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

pub trait McpDeploymentProvider: Send + Sync {
    fn load_config(&self) -> impl Future<Output = McpRegistryResult<ServicesConfig>> + Send;

    fn protocol_version(&self) -> &str;
}
