//! Rate limit configuration.

use crate::profile::RateLimitsConfig;

#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    pub oauth_public_per_second: u64,
    pub oauth_auth_per_second: u64,
    pub contexts_per_second: u64,
    pub tasks_per_second: u64,
    pub artifacts_per_second: u64,
    pub agent_registry_per_second: u64,
    pub agents_per_second: u64,
    pub mcp_registry_per_second: u64,
    pub mcp_per_second: u64,
    pub stream_per_second: u64,
    pub content_per_second: u64,
    pub burst_multiplier: u64,
    pub disabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            oauth_public_per_second: 2,
            oauth_auth_per_second: 2,
            contexts_per_second: 50,
            tasks_per_second: 10,
            artifacts_per_second: 15,
            agent_registry_per_second: 20,
            agents_per_second: 3,
            mcp_registry_per_second: 20,
            mcp_per_second: 100,
            stream_per_second: 1,
            content_per_second: 20,
            burst_multiplier: 2,
            disabled: false,
        }
    }
}

impl RateLimitConfig {
    pub fn production() -> Self {
        Self::default()
    }

    pub const fn testing() -> Self {
        Self {
            oauth_public_per_second: 10000,
            oauth_auth_per_second: 10000,
            contexts_per_second: 10000,
            tasks_per_second: 10000,
            artifacts_per_second: 10000,
            agent_registry_per_second: 10000,
            agents_per_second: 10000,
            mcp_registry_per_second: 10000,
            mcp_per_second: 10000,
            stream_per_second: 10000,
            content_per_second: 10000,
            burst_multiplier: 100,
            disabled: false,
        }
    }

    pub const fn disabled() -> Self {
        let mut config = Self::testing();
        config.disabled = true;
        config
    }
}

impl From<&RateLimitsConfig> for RateLimitConfig {
    fn from(config: &RateLimitsConfig) -> Self {
        Self {
            oauth_public_per_second: config.oauth_public_per_second,
            oauth_auth_per_second: config.oauth_auth_per_second,
            contexts_per_second: config.contexts_per_second,
            tasks_per_second: config.tasks_per_second,
            artifacts_per_second: config.artifacts_per_second,
            agent_registry_per_second: config.agent_registry_per_second,
            agents_per_second: config.agents_per_second,
            mcp_registry_per_second: config.mcp_registry_per_second,
            mcp_per_second: config.mcp_per_second,
            stream_per_second: config.stream_per_second,
            content_per_second: config.content_per_second,
            burst_multiplier: config.burst_multiplier,
            disabled: config.disabled,
        }
    }
}
