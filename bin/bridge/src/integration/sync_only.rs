//! Agents the gateway governs centrally, with nothing to install locally.
//!
//! `claude-code` is enabled in the instance manifest exactly like the desktop
//! hosts, but it has no [`crate::integration::HostApp`] — it reaches the
//! gateway itself and only receives skill/plugin sync from here — and it is
//! still listed on the Agents card.
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

// Why: Claude Code with no gateway keys in the settings it reads keeps the
// user's own Anthropic login, so reporting it Working would hide exactly the
// fault `doctor`'s "claude code routing" check fails on.
#[must_use]
pub fn gateway_routed(agent: &SyncOnlyAgent) -> bool {
    agent.id != "claude-code" || super::claude_code_routing::is_routed()
}

pub const fn sync_only_verdict(manifest_synced: bool, gateway_routed: bool) -> AgentVerdict {
    if !manifest_synced {
        return AgentVerdict {
            state: AgentState::Checking,
            tone: AgentState::Checking.tone(),
            reason: AgentReason::NeverProbed,
            action: None,
            is_set_up: false,
            is_installed: false,
            is_running: false,
        };
    }
    if !gateway_routed {
        return AgentVerdict {
            state: AgentState::Attention,
            tone: AgentState::Attention.tone(),
            reason: AgentReason::NotRouted,
            action: None,
            is_set_up: true,
            is_installed: false,
            is_running: false,
        };
    }
    AgentVerdict {
        state: AgentState::Working,
        tone: AgentState::Working.tone(),
        reason: AgentReason::CloudManaged,
        action: None,
        is_set_up: true,
        is_installed: true,
        is_running: false,
    }
}
