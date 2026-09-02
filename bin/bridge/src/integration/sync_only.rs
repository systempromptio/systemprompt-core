//! Agents the gateway governs centrally, with nothing to install locally.
//!
//! `claude-code` is enabled in the instance manifest exactly like the desktop
//! hosts, but it has no [`crate::integration::HostApp`] — it reaches the
//! gateway itself and only receives skill/plugin sync from here.
//! Before this table they were simply invisible: a user running Claude Code
//! looked at the Agents card and saw no sign of the agent they were using.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::agent_health::{AgentReason, AgentState, AgentVerdict};

#[derive(Debug, Clone, Copy)]
pub struct SyncOnlyAgent {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
}

// Why: kept in step with `KNOWN_HOSTS` in
// `systemprompt_models::bridge::profile`; the inventory test asserts the two
// cover each other.
pub const SYNC_ONLY_AGENTS: &[SyncOnlyAgent] = &[SyncOnlyAgent {
    id: "claude-code",
    display_name: "Claude Code",
    description: "Governed through the gateway; skills and plugins sync from here.",
    icon: "claude-code",
}];

#[must_use]
pub fn sync_only_agent(host_id: &str) -> Option<&'static SyncOnlyAgent> {
    SYNC_ONLY_AGENTS.iter().find(|a| a.id == host_id)
}

// Why: a sync-only agent is governed by construction — it reaches the gateway
// directly — so the only thing this machine can say about it is whether the
// manifest that enables it has arrived yet.
pub const fn sync_only_verdict(manifest_synced: bool) -> AgentVerdict {
    if manifest_synced {
        AgentVerdict {
            state: AgentState::Working,
            tone: AgentState::Working.tone(),
            reason: AgentReason::CloudManaged,
            action: None,
            is_set_up: true,
            is_installed: true,
            is_running: false,
        }
    } else {
        AgentVerdict {
            state: AgentState::Checking,
            tone: AgentState::Checking.tone(),
            reason: AgentReason::NeverProbed,
            action: None,
            is_set_up: false,
            is_installed: false,
            is_running: false,
        }
    }
}
