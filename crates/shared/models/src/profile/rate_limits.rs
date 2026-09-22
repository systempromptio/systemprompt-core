//! Rate limits configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RateLimitsConfig {
    #[serde(default)]
    pub disabled: bool,

    #[serde(default = "default_oauth_public")]
    pub oauth_public_per_second: u64,

    #[serde(default = "default_oauth_auth")]
    pub oauth_auth_per_second: u64,

    #[serde(default = "default_contexts")]
    pub contexts_per_second: u64,

    #[serde(default = "default_tasks")]
    pub tasks_per_second: u64,

    #[serde(default = "default_artifacts")]
    pub artifacts_per_second: u64,

    #[serde(default = "default_agent_registry")]
    pub agent_registry_per_second: u64,

    #[serde(default = "default_agents")]
    pub agents_per_second: u64,

    #[serde(default = "default_mcp_registry")]
    pub mcp_registry_per_second: u64,

    #[serde(default = "default_mcp")]
    pub mcp_per_second: u64,

    #[serde(default = "default_stream")]
    pub stream_per_second: u64,

    #[serde(default = "default_content")]
    pub content_per_second: u64,

    #[serde(default = "default_gateway")]
    pub gateway_per_second: u64,

    /// Budget for `/v1/auth/bridge/*`, kept separate from `gateway_per_second`.
    ///
    /// Why: sign-in is low-volume and must stay reachable on an instance whose
    /// inference traffic is saturating its own budget. Sharing one bucket let a
    /// busy gateway lock every user out of authenticating.
    #[serde(default = "default_bridge_auth")]
    pub bridge_auth_per_second: u64,

    #[serde(default = "default_burst")]
    pub burst_multiplier: u64,
}

pub const fn default_oauth_public() -> u64 {
    10
}
pub const fn default_oauth_auth() -> u64 {
    10
}
pub const fn default_contexts() -> u64 {
    100
}
pub const fn default_tasks() -> u64 {
    50
}
pub const fn default_artifacts() -> u64 {
    50
}
pub const fn default_agent_registry() -> u64 {
    50
}
pub const fn default_agents() -> u64 {
    20
}
pub const fn default_mcp_registry() -> u64 {
    50
}
pub const fn default_mcp() -> u64 {
    200
}
pub const fn default_stream() -> u64 {
    100
}
pub const fn default_content() -> u64 {
    50
}
pub const fn default_gateway() -> u64 {
    100
}
pub const fn default_bridge_auth() -> u64 {
    20
}
pub const fn default_burst() -> u64 {
    3
}

impl Default for RateLimitsConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            oauth_public_per_second: default_oauth_public(),
            oauth_auth_per_second: default_oauth_auth(),
            contexts_per_second: default_contexts(),
            tasks_per_second: default_tasks(),
            artifacts_per_second: default_artifacts(),
            agent_registry_per_second: default_agent_registry(),
            agents_per_second: default_agents(),
            mcp_registry_per_second: default_mcp_registry(),
            mcp_per_second: default_mcp(),
            stream_per_second: default_stream(),
            content_per_second: default_content(),
            gateway_per_second: default_gateway(),
            bridge_auth_per_second: default_bridge_auth(),
            burst_multiplier: default_burst(),
        }
    }
}
