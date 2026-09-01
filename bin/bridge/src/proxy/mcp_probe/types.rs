//! The MCP probe's wire types: one server's auth outcome, its tools, and
//! the state that outcome is folded into.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::verdict::{Tone, Verdict};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct McpTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct McpServerAuth {
    pub id: String,
    pub url: String,
    pub state: McpAuthState,
    pub tools: Vec<McpTool>,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(type = "string | null"))]
    pub session_id: Option<crate::ids::McpSessionId>,
    pub probed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum McpAuthState {
    #[default]
    Unknown,
    NoServers,
    Authenticated,
    LoopbackMismatch,
    GatewayUnauthorized,
    NotRegistered,
    UpstreamError,
    ProxyUnreachable,
    // Why: The probe ran out of time. Distinct from `ProxyUnreachable`: a server
    // too slow to answer in six seconds is not a server that is down, and is
    // certainly not one that needs signing in to.
    ProbeTimeout,
    // Why: Something on *this* machine stopped the probe before it reached the
    // server -- no HTTP client, no loopback secret. Says nothing about the
    // server at all.
    LocalError,
    ProtocolError,
}

impl McpAuthState {
    // Why: The single answer to "must the user sign in to this server again?".
    //
    // Why one function: this predicate drives the desktop notification and the
    // per-server panel. When each surface derived it separately they disagreed,
    // and the UI told users to re-auth four healthy servers.
    #[must_use]
    pub const fn needs_sign_in(self) -> bool {
        matches!(self, Self::GatewayUnauthorized | Self::NotRegistered)
    }

    #[must_use]
    pub const fn is_conclusive(self) -> bool {
        !matches!(
            self,
            Self::Unknown | Self::ProxyUnreachable | Self::ProbeTimeout | Self::LocalError
        )
    }

    // Why: an inconclusive probe is *unknown*, never red. The Status pane used
    // to paint `ProxyUnreachable` as a failure of the server, which it is not.
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Authenticated => Tone::Ok,
            Self::NoServers => Tone::Warn,
            Self::Unknown | Self::ProxyUnreachable | Self::ProbeTimeout | Self::LocalError => {
                Tone::Unknown
            },
            Self::LoopbackMismatch
            | Self::GatewayUnauthorized
            | Self::NotRegistered
            | Self::UpstreamError
            | Self::ProtocolError => Tone::Err,
        }
    }

    #[must_use]
    pub const fn shows_tools(self) -> bool {
        matches!(self, Self::Authenticated)
    }

    #[must_use]
    pub const fn verdict(self) -> Verdict<Self> {
        Verdict::new(self.tone(), self)
    }
}
