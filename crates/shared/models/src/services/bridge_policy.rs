//! Instance-level policy the bridge enforces on managed client installations.
//!
//! Configured as a top-level `bridge_policy:` section in a services YAML and
//! carried to clients inside the signed bridge manifest. Two knobs today:
//! whether Claude Code's managed-MCP policy re-allows claude.ai first-party
//! connectors (`allowAllClaudeAiMcps`) alongside the managed server set —
//! without it, writing `managed-mcp.json` suppresses every connector the user
//! linked on claude.ai — and whether bridges keep themselves current.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgePolicyConfig {
    #[serde(default)]
    pub allow_claude_ai_connectors: bool,
    #[serde(default)]
    pub auto_update: AutoUpdatePolicy,
}

/// Whether a bridge updates itself, and how far it is allowed to go on its own.
///
/// `Staged` downloads, verifies and swaps the on-disk binary but never restarts
/// the running process: the next natural launch runs the new version. There is
/// deliberately no variant that restarts unattended — the fleet-wide brake for
/// a bad release is `pinned_version` on the release feed, not a client toggle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoUpdatePolicy {
    Disabled,
    #[default]
    Staged,
}

impl AutoUpdatePolicy {
    #[must_use]
    pub const fn stages(self) -> bool {
        matches!(self, Self::Staged)
    }
}
