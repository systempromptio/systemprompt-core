//! External-agent ("super-agent") catalog config.
//!
//! External agents are native apps and CLI tools that run **off** our
//! infrastructure (Claude Desktop, Codex CLI, Claude Code) and connect to the
//! gateway via the systemprompt-bridge binary
//! (`systemprompt-core/bin/bridge`). They are intentionally distinct from
//! [`AgentConfig`](super::AgentConfig), which describes server-side A2A agents
//! we run.
//!
//! The bridge owns its own static `HostApp` registry; this gateway-side YAML
//! is the catalog the admin UI lists, the operator's `enabled` flag, and
//! `default_enabled: false` for a host each user must turn on.
//! The `id` field MUST equal the bridge's `HostApp::id()` for the same host
//! (e.g. `claude_desktop`, `codex_cli`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ExternalAgentId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAgentKind {
    DesktopApp,
    CliTool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalAgentConfig {
    pub id: ExternalAgentId,
    pub display_name: String,
    pub kind: ExternalAgentKind,
    pub enabled: bool,
    /// Whether the host reaches a user who has not chosen. `false` makes an
    /// enabled host opt-in: it reaches only users who turn it on.
    #[serde(default = "default_enabled")]
    pub default_enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub docs_url: Option<String>,
    #[serde(default)]
    pub desktop_policy: systemprompt_models::bridge::desktop_policy::DesktopPolicy,
}

const fn default_enabled() -> bool {
    true
}
