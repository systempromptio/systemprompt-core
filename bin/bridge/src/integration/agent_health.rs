//! Agent-health classification from typed integration state.
//!
//! Serialized verdict codes select localized GUI messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::integration::host_app::{AppInstallState, ProfileState};

use crate::proxy_probe::ProxyProbeState;

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
            secondary_action: None,
            is_set_up: false,
            is_listed: false,
            is_installed: false,
            is_running: false,
        };
    };

    let is_set_up = !matches!(snap.profile_state, ProfileState::Absent);
    let is_installed = matches!(snap.profile_state, ProfileState::Installed);
    let is_running = snap.host_running.unwrap_or(false);
    let is_listed = is_set_up
        || snap.app_installed == AppInstallState::Installed
        || input.pending != PendingStep::None;
    let finish = |state: AgentState, reason, action| AgentVerdict {
        state,
        tone: state.tone(),
        reason,
        action,
        secondary_action: None,
        is_set_up,
        is_listed,
        is_installed,
        is_running,
    };

    if snap.app_installed == AppInstallState::NotInstalled {
        let download = input.has_download_url.then_some(AgentAction::Download);
        if !is_set_up {
            return finish(AgentState::NotInstalled, AgentReason::AppMissing, download);
        }
        return AgentVerdict {
            secondary_action: Some(AgentAction::RemoveLeftover),
            ..finish(
                AgentState::NotInstalled,
                AgentReason::LeftoverConfig,
                download,
            )
        };
    }

    if matches!(input.pending, PendingStep::OsApproval { .. })
        && matches!(snap.profile_state, ProfileState::Absent)
    {
        if input.pending.approval_expired(snap.probed_at_unix) {
            return AgentVerdict {
                secondary_action: Some(AgentAction::ShowProfileFile),
                ..finish(
                    AgentState::ApprovalExpired,
                    AgentReason::ApprovalExpired,
                    Some(AgentAction::OfferAgain),
                )
            };
        }
        return finish(
            AgentState::NeedsApproval,
            AgentReason::AwaitingOsApproval,
            Some(AgentAction::OpenDeviceManagement),
        );
    }

    if let Some((state, reason, action)) = profile_finding(snap) {
        return finish(state, reason, Some(action));
    }

    if let Some(failed) = apply_failed(input.apply_failures) {
        let state = if failed.1 == Some(AgentAction::UpdateAdmin) {
            AgentState::NeedsAdminApproval
        } else {
            AgentState::Attention
        };
        return finish(state, failed.0, failed.1);
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

    if input.pending == PendingStep::Restart && is_running {
        return finish(
            AgentState::RestartNeeded,
            AgentReason::RestartPending,
            Some(AgentAction::Restart),
        );
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

fn profile_finding(
    snap: &crate::integration::HostAppSnapshot,
) -> Option<(AgentState, AgentReason, AgentAction)> {
    match &snap.profile_state {
        ProfileState::Stale { reason, keys } => {
            let action = if reason.breaks_routing() {
                AgentAction::Repair
            } else {
                update_action(snap)
            };
            let state = if action == AgentAction::UpdateAdmin {
                AgentState::NeedsAdminApproval
            } else {
                AgentState::NeedsReapply
            };
            Some((
                state,
                AgentReason::Stale {
                    cause: *reason,
                    keys: keys.join(", "),
                },
                action,
            ))
        },
        ProfileState::Partial { missing_required } => Some((
            AgentState::Attention,
            AgentReason::Partial {
                missing: missing_required.join(", "),
            },
            AgentAction::Repair,
        )),
        ProfileState::Unverifiable { reason } => Some((
            AgentState::Attention,
            AgentReason::Unverifiable {
                detail: reason.clone(),
            },
            AgentAction::Verify,
        )),
        ProfileState::Absent => Some((
            AgentState::NotSetUp,
            AgentReason::Absent,
            AgentAction::Connect,
        )),
        ProfileState::Installed => None,
    }
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
    OS_APPROVAL_WINDOW_SECS, PendingStep,
};
pub use super::agent_verdict::{AgentAction, AgentReason, AgentState, AgentVerdict};
pub use super::sync_only::{SYNC_ONLY_AGENTS, SyncOnlyAgent, sync_only_agent};
