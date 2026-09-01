//! Agents the gateway governs centrally, with nothing to install locally.
//!
//! `claude-code` and `cowork` are enabled in the instance manifest exactly like
//! the desktop hosts, but they have no [`crate::integration::HostApp`] — they
//! reach the gateway themselves and only receive skill/plugin sync from here.
//! Before this table they were simply invisible: a user running Claude Code
//! looked at the Agents card and saw no sign of the agent they were using.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
pub const SYNC_ONLY_AGENTS: &[SyncOnlyAgent] = &[
    SyncOnlyAgent {
        id: "claude-code",
        display_name: "Claude Code",
        description: "Governed through the gateway; skills and plugins sync from here.",
        icon: "claude-code",
    },
    SyncOnlyAgent {
        id: "cowork",
        display_name: "Cowork",
        description: "Governed through the gateway; artifacts and plugins sync from here.",
        icon: "cowork",
    },
];

#[must_use]
pub fn sync_only_agent(host_id: &str) -> Option<&'static SyncOnlyAgent> {
    SYNC_ONLY_AGENTS.iter().find(|a| a.id == host_id)
}
