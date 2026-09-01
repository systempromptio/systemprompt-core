//! Host-app integrations: Claude Desktop, Cowork artifacts, launch helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_fleet;
pub mod agent_health;
pub(crate) mod app_launch;
pub mod claude_code_cli;
pub mod claude_desktop;
pub mod codex_cli;
pub(crate) mod config_read;
pub mod cowork_artifacts;
pub mod cowork_plugins;
pub mod hermes;
pub mod host_app;
pub(crate) mod managed_skills;
pub mod opencode;
pub mod profile_state;
pub mod reapply;
pub mod registry;
#[cfg(feature = "dev-stub-host")]
pub mod stub_host;
pub mod sync_only;
pub mod uninstall;

pub use crate::proxy_probe::{ProxyHealth, ProxyProbeState};
pub use agent_health::{
    AgentAction, AgentFleetSummary, AgentFleets, AgentReason, AgentState, AgentSurface,
    AgentVerdict, FleetHeadline, FleetState, HostHealthInputs, HostModelViewRef, SYNC_ONLY_AGENTS,
    SyncOnlyAgent, sync_only_agent, verdict,
};
pub use host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema,
    ProfileGenInputs, ProfileState, StaleReason,
};
pub use registry::{find_host_by_id, host_apps};
