//! Agent-health classification from typed integration state.
//!
//! Serialized verdict codes select localized GUI messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::integration::host_app::{AppInstallState, ProfileState, StaleReason};

use crate::proxy_probe::ProxyProbeState;
use crate::verdict::Tone;

/// Host configuration, application and proxy health state.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentState {
    Working,
    Ready,
    Attention,
    NotSetUp,
    NotInstalled,
    Down,
    Checking,
}

impl AgentState {
    // Why: an app that is not on this computer is a fact about the computer,
    // not a fault in the agent, so it reads neutral and never asks for
    // attention.
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Working | Self::Ready => Tone::Ok,
            Self::Attention | Self::NotSetUp => Tone::Warn,
            Self::Down => Tone::Err,
            Self::Checking | Self::NotInstalled => Tone::Unknown,
        }
    }
}

/// Why the agent is in that state, with the arguments its message needs.
///
/// `CloudManaged` is routed through the gateway centrally; `NotRouted` is a
/// sync-only CLI whose settings do not send its inference to the gateway.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentReason {
    Governed { when_unix: Option<u64> },
    Awaiting,
    AppMissing,
    Stale { cause: StaleReason, keys: String },
    SyncFailed { detail: String },
    Partial { missing: String },
    Unverifiable { detail: String },
    Absent,
    NoKey { providers: String },
    NoModels,
    ProxyDown { probe: ProxyProbeState },
    NeverProbed,
    CloudManaged,
    NotRouted,
}

/// The one button that fixes this state, if there is one.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentAction {
    Download,
    Repair,
    Update,
    UpdateAdmin,
    Verify,
    Open,
    Add,
    Enrol,
}

/// The verdict for one agent, plus the three facts the GUI renders around it.
///
/// `is_set_up` means the agent belongs in the Agents list rather than behind
/// "Add agent" (merely "not absent", so partial and stale profiles count);
/// `is_installed` means the managed profile is present and complete;
/// `is_running` is the raw process-table fact — the app is open — and says
/// nothing about whether it is configured, so it is never the headline.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct AgentVerdict {
    pub state: AgentState,
    pub tone: Tone,
    pub reason: AgentReason,
    pub action: Option<AgentAction>,
    pub is_set_up: bool,
    pub is_installed: bool,
    pub is_running: bool,
}

#[must_use]
pub fn verdict(input: &HostHealthInputs<'_>) -> AgentVerdict {
    if input.surface == AgentSurface::SyncOnly {
        return super::sync_only::sync_only_verdict(
            input.manifest_synced,
            input.gateway_routed,
            input.apply_failures,
        );
    }

    let Some(snap) = input.snapshot else {
        return AgentVerdict {
            state: AgentState::Checking,
            tone: AgentState::Checking.tone(),
            reason: AgentReason::NeverProbed,
            action: None,
            is_set_up: false,
            is_installed: false,
            is_running: false,
        };
    };

    let is_set_up = !matches!(snap.profile_state, ProfileState::Absent);
    let is_installed = matches!(snap.profile_state, ProfileState::Installed);
    let is_running = snap.host_running.unwrap_or(false);
    let finish = |state: AgentState, reason, action| AgentVerdict {
        state,
        tone: state.tone(),
        reason,
        action,
        is_set_up,
        is_installed,
        is_running,
    };

    if snap.app_installed == AppInstallState::NotInstalled {
        return finish(
            AgentState::NotInstalled,
            AgentReason::AppMissing,
            input.has_download_url.then_some(AgentAction::Download),
        );
    }

    match &snap.profile_state {
        ProfileState::Stale { reason, keys } => {
            let action = if reason.breaks_routing() {
                AgentAction::Repair
            } else {
                update_action(snap)
            };
            return finish(
                AgentState::Attention,
                AgentReason::Stale {
                    cause: *reason,
                    keys: keys.join(", "),
                },
                Some(action),
            );
        },
        ProfileState::Partial { missing_required } => {
            return finish(
                AgentState::Attention,
                AgentReason::Partial {
                    missing: missing_required.join(", "),
                },
                Some(AgentAction::Repair),
            );
        },
        ProfileState::Unverifiable { reason } => {
            return finish(
                AgentState::Attention,
                AgentReason::Unverifiable {
                    detail: reason.clone(),
                },
                Some(AgentAction::Verify),
            );
        },
        ProfileState::Absent => {
            return finish(
                AgentState::NotSetUp,
                AgentReason::Absent,
                Some(AgentAction::Add),
            );
        },
        ProfileState::Installed => {},
    }

    if let Some(failed) = apply_failed(input.apply_failures) {
        return finish(AgentState::Attention, failed.0, failed.1);
    }

    if input.models.checked && !input.models.available {
        let reason = if input.models.unconfigured_providers.is_empty() {
            AgentReason::NoModels
        } else {
            AgentReason::NoKey {
                providers: input.models.unconfigured_providers.join(", "),
            }
        };
        return finish(AgentState::Attention, reason, None);
    }

    let open = input.can_open.then_some(AgentAction::Open);
    match input.proxy.state {
        ProxyProbeState::Unconfigured => finish(AgentState::Ready, AgentReason::Awaiting, open),
        ProxyProbeState::Listening => finish(
            AgentState::Working,
            AgentReason::Governed {
                when_unix: (snap.probed_at_unix > 0).then_some(snap.probed_at_unix),
            },
            open,
        ),
        ProxyProbeState::Unknown => finish(AgentState::Checking, AgentReason::NeverProbed, None),
        probe => finish(
            AgentState::Down,
            AgentReason::ProxyDown { probe },
            Some(AgentAction::Verify),
        ),
    }
}

#[must_use]
pub fn apply_failed(failures: &[ApplyFailure]) -> Option<(AgentReason, Option<AgentAction>)> {
    if failures.is_empty() {
        return None;
    }
    let detail = failures
        .iter()
        .map(|f| format!("{}: {}", f.emitter, f.error))
        .collect::<Vec<_>>()
        .join("; ");
    let action = failures
        .iter()
        .all(|f| f.needs_elevation)
        .then_some(AgentAction::UpdateAdmin);
    Some((AgentReason::SyncFailed { detail }, action))
}

const fn update_action(snap: &crate::integration::HostAppSnapshot) -> AgentAction {
    if snap.update_needs_approval {
        AgentAction::UpdateAdmin
    } else {
        AgentAction::Update
    }
}

pub use super::agent_fleet::{AgentFleetSummary, AgentFleets, FleetHeadline, FleetState};
pub use super::agent_inputs::{
    AgentSurface, ApplyFailure, HostCapabilities, HostHealthInputs, HostModelViewRef,
};
pub use super::sync_only::{SYNC_ONLY_AGENTS, SyncOnlyAgent, sync_only_agent};
