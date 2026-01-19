//! Rate limits configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TierMultipliers {
    #[serde(default = "default_admin_multiplier")]
    pub admin: f64,

    #[serde(default = "default_user_multiplier")]
    pub user: f64,

    #[serde(default = "default_a2a_multiplier")]
    pub a2a: f64,

    #[serde(default = "default_mcp_multiplier")]
    pub mcp: f64,

    #[serde(default = "default_service_multiplier")]
    pub service: f64,

    #[serde(default = "default_anon_multiplier")]
    pub anon: f64,
}

pub const fn default_admin_multiplier() -> f64 {
    10.0
}
pub const fn default_user_multiplier() -> f64 {
    1.0
}
pub const fn default_a2a_multiplier() -> f64 {
    5.0
}
pub const fn default_mcp_multiplier() -> f64 {
    5.0
}
pub const fn default_service_multiplier() -> f64 {
    5.0
}
pub const fn default_anon_multiplier() -> f64 {
    0.5
}

impl Default for TierMultipliers {
    fn default() -> Self {
        Self {
            admin: default_admin_multiplier(),
            user: default_user_multiplier(),
            a2a: default_a2a_multiplier(),
            mcp: default_mcp_multiplier(),
            service: default_service_multiplier(),
            anon: default_anon_multiplier(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

    #[serde(default = "default_burst")]
    pub burst_multiplier: u64,

    #[serde(default)]
    pub tier_multipliers: TierMultipliers,
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
            burst_multiplier: default_burst(),
            tier_multipliers: TierMultipliers::default(),
        }
    }
}
