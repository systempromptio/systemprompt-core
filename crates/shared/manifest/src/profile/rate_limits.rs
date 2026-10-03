//! Rate limits configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// Per-route-group request budgets, in requests per second.
///
/// Each `*_per_second` field governs one route group:
///
/// - `oauth_public` — `/api/v1/core/oauth` public endpoints,
///   `/auth/link-passkey`, and the public gateway session routes under
///   `/api/public/gateway`.
/// - `oauth_auth` — the authenticated half of `/api/v1/core/oauth`, plus
///   `/api/v1/core/users`.
/// - `contexts` — `/api/v1/core/contexts` and the inbound `/api/v1/webhook`
///   mount.
/// - `tasks` — `/api/v1/core/tasks`.
/// - `artifacts` — `/api/v1/core/artifacts`.
/// - `agent_registry` — `/api/v1/agents/registry`, discovery only, kept apart
///   from `agents_per_second` so listing agents cannot spend an execution
///   budget.
/// - `agents` — `/api/v1/agents` execution, and the Slack and Teams inbound
///   mounts that dispatch to an agent.
/// - `mcp_registry` — `/api/v1/mcp/registry`, the MCP server catalog, discovery
///   only.
/// - `mcp` — `/api/v1/mcp` tool calls, the busiest protocol route group.
/// - `stream` — `/api/v1/stream`, long-lived SSE connections, so this is a
///   connection budget rather than a request one and is set high.
/// - `content` — `/api/v1/content`, `/api/v1/sync`, `/api/v1/marketplace`,
///   `/api/v1/analytics` and `/track/engagement`.
/// - `gateway` — inference traffic under `/v1`: `/v1/messages` and the
///   OpenAI-shaped `/v1/chat/completions`.
/// - `bridge_auth` — `/v1/auth/bridge/*`, deliberately separate from
///   `gateway_per_second`: sign-in is low-volume and must stay reachable on an
///   instance whose inference traffic is saturating its own budget. Sharing one
///   bucket let a busy gateway lock every user out of authenticating.
///
/// `per_second_budgets` projects those fields as a list; both validators
/// iterate it rather than naming the fields themselves, which had already let
/// them drift apart and let a zero in an unnamed field through.
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

impl RateLimitsConfig {
    #[must_use]
    pub const fn per_second_budgets(&self) -> [(&'static str, u64); 13] {
        [
            ("oauth_public_per_second", self.oauth_public_per_second),
            ("oauth_auth_per_second", self.oauth_auth_per_second),
            ("contexts_per_second", self.contexts_per_second),
            ("tasks_per_second", self.tasks_per_second),
            ("artifacts_per_second", self.artifacts_per_second),
            ("agent_registry_per_second", self.agent_registry_per_second),
            ("agents_per_second", self.agents_per_second),
            ("mcp_registry_per_second", self.mcp_registry_per_second),
            ("mcp_per_second", self.mcp_per_second),
            ("stream_per_second", self.stream_per_second),
            ("content_per_second", self.content_per_second),
            ("gateway_per_second", self.gateway_per_second),
            ("bridge_auth_per_second", self.bridge_auth_per_second),
        ]
    }

    #[must_use]
    pub fn production() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn testing() -> Self {
        Self {
            disabled: false,
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
            gateway_per_second: 10000,
            bridge_auth_per_second: 10000,
            burst_multiplier: 100,
        }
    }

    #[must_use]
    pub const fn disabled() -> Self {
        let mut config = Self::testing();
        config.disabled = true;
        config
    }
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
