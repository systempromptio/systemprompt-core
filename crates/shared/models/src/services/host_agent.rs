//! Host-agent ("super-agent") catalog config.
//!
//! Host-agents are native apps and CLI tools that run on a user's machine and
//! connect to the gateway via the systemprompt-bridge binary
//! (`systemprompt-core/bin/bridge`). This is intentionally distinct from
//! [`AgentConfig`](super::AgentConfig), which describes server-side A2A agents.
//!
//! The bridge owns its own static `HostApp` registry; this gateway-side YAML
//! is the catalog the admin UI lists and the operator's `enabled` flag.
//! The `id` field MUST equal the bridge's `HostApp::id()` for the same host
//! (e.g. `claude_desktop`, `codex_cli`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostAgentKind {
    DesktopApp,
    CliTool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostAgentConfig {
    pub id: String,
    pub display_name: String,
    pub kind: HostAgentKind,
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub docs_url: Option<String>,
}
