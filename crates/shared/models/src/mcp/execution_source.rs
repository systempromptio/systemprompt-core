//! Where a tool execution was observed, and how surely its rows were joined.
//!
//! Every tool call the platform records is seen from one of a fixed set of
//! vantage points. [`ExecutionSource`] names that vantage point on the
//! execution row so an operator can tell a server-observed run from a
//! client-reported one. [`Correlation`] states whether the execution was
//! joined to its intent and artifact by an exact key or by inference — an
//! inferred join is a visible state, never a silent match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The vantage point the platform saw a tool result from.
///
/// In-process executor, the HTTP proxy tapping an external server, a
/// `tool_result` block replayed in a `/v1/messages` history, or a client
/// host's tool-completion hook (`PostToolUse` for Claude Code and Cowork;
/// `OpenCode`'s own hook).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionSource {
    InProcess,
    Proxy,
    Gateway,
    HookClaudeCode,
    HookOpenCode,
}

impl ExecutionSource {
    pub const ALL: [Self; 5] = [
        Self::InProcess,
        Self::Proxy,
        Self::Gateway,
        Self::HookClaudeCode,
        Self::HookOpenCode,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::Proxy => "proxy",
            Self::Gateway => "gateway",
            Self::HookClaudeCode => "hook_claude_code",
            Self::HookOpenCode => "hook_opencode",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }

    #[must_use]
    pub const fn is_server_observed(self) -> bool {
        matches!(self, Self::InProcess | Self::Proxy)
    }

    #[must_use]
    pub const fn from_hook_host(host: &str) -> Self {
        if host.eq_ignore_ascii_case("opencode") {
            Self::HookOpenCode
        } else {
            Self::HookClaudeCode
        }
    }
}

impl std::fmt::Display for ExecutionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a tool result was joined to its invocation.
///
/// `Exact` by a key both sides carried (the client `tool_use_id` or the
/// server `mcp_execution_id`); `Inferred` by session, tool name, payload
/// digest and time — the only option when a client host dropped every id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Correlation {
    Exact,
    Inferred,
}

impl Correlation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Inferred => "inferred",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
            "inferred" => Some(Self::Inferred),
            _ => None,
        }
    }
}

impl std::fmt::Display for Correlation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
