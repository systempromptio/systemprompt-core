//! Instance-level policy the bridge enforces on managed client installations.
//!
//! Configured as a top-level `bridge_policy:` section in a services YAML and
//! carried to clients inside the signed bridge manifest. The knobs:
//! whether Claude Code's managed-MCP policy re-allows claude.ai first-party
//! connectors (`allowAllClaudeAiMcps`) alongside the managed server set —
//! without it, writing `managed-mcp.json` suppresses every connector the user
//! linked on claude.ai — whether bridges keep themselves current, and whether
//! a user may switch the bridge off to run the native apps on a personal
//! subscription.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_models::bridge::update_policy::AutoUpdatePolicy;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgePolicyConfig {
    #[serde(default)]
    pub allow_claude_ai_connectors: bool,
    #[serde(default)]
    pub auto_update: AutoUpdatePolicy,
    #[serde(default)]
    pub allow_user_disable: bool,
}
