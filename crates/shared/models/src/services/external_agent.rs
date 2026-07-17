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
//! is the catalog the admin UI lists and the operator's `enabled` flag.
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
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub docs_url: Option<String>,
}
