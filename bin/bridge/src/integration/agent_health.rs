//! Agent-health classification from typed integration state.
//!
//! Serialized verdict codes select localized GUI messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::integration::host_app::{AppInstallState, ProfileState};

use crate::proxy_probe::ProxyProbeState;

struct Finding {
    state: AgentState,
    reason: AgentReason,
    action: Option<AgentAction>,
    secondary_action: Option<AgentAction>,
}

impl Finding {
    const fn new(state: AgentState, reason: AgentReason, action: Option<AgentAction>) -> Self {
        Self {
            state,
            reason,
            action,
            secondary_action: None,
        }
    }

    const fn with_secondary(mut self, action: AgentAction) -> Self {
        self.secondary_action = Some(action);
        self
    }
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
            secondary_action: None,
            is_set_up: false,
            is_listed: false,
            is_installed: false,
            is_running: false,
        };
    };

    let is_set_up = !matches!(snap.profile_state, ProfileState::Absent);
    let is_running = snap.host_running.unwrap_or(false);
    let finding = app_missing(input, snap, is_set_up)
        .or_else(|| os_approval(input, snap))
        .or_else(|| profile_finding(snap))
        .or_else(|| apply_failure(input.apply_failures))
        .or_else(|| model_finding(&input.models))
        .or_else(|| restart_finding(input, is_running))
        .unwrap_or_else(|| proxy_finding(input, snap));
    AgentVerdict {
        state: finding.state,
        tone: finding.state.tone(),
        reason: finding.reason,
        action: finding.action,
        secondary_action: finding.secondary_action,
        is_set_up,
        is_listed: is_set_up
            || snap.app_installed == AppInstallState::Installed
            || input.pending != PendingStep::None,
        is_installed: matches!(snap.profile_state, ProfileState::Installed),
        is_running,
    }
}

fn app_missing(
    input: &HostHealthInputs<'_>,
    snap: &crate::integration::HostAppSnapshot,
    is_set_up: bool,
) -> Option<Finding> {
    if snap.app_installed != AppInstallState::NotInstalled {
        return None;
    }
    let download = input.has_download_url.then_some(AgentAction::Download);
    if !is_set_up {
        return Some(Finding::new(
            AgentState::NotInstalled,
            AgentReason::AppMissing,
            download,
        ));
    }
    Some(
        Finding::new(
            AgentState::NotInstalled,
            AgentReason::LeftoverConfig,
            download,
        )
        .with_secondary(AgentAction::RemoveLeftover),
    )
}

const fn os_approval(
    input: &HostHealthInputs<'_>,
    snap: &crate::integration::HostAppSnapshot,
) -> Option<Finding> {
    if !matches!(input.pending, PendingStep::OsApproval { .. })
        || !matches!(snap.profile_state, ProfileState::Absent)
    {
        return None;
    }
    if input.pending.approval_expired(snap.probed_at_unix) {
        return Some(
            Finding::new(
                AgentState::ApprovalExpired,
                AgentReason::ApprovalExpired,
                Some(AgentAction::OfferAgain),
            )
            .with_secondary(AgentAction::ShowProfileFile),
        );
    }
    Some(Finding::new(
        AgentState::NeedsApproval,
        AgentReason::AwaitingOsApproval,
        Some(AgentAction::OpenDeviceManagement),
    ))
}

fn apply_failure(failures: &[ApplyFailure]) -> Option<Finding> {
    let (reason, action) = apply_failed(failures)?;
    let state = if action == Some(AgentAction::UpdateAdmin) {
        AgentState::NeedsAdminApproval
    } else {
        AgentState::Attention
    };
    Some(Finding::new(state, reason, action))
}

fn model_finding(models: &HostModelViewRef<'_>) -> Option<Finding> {
    if !models.checked {
        return None;
    }
    // Why: a personal-subscription provider (OpenCode Go) is advertised but
    // serves nothing until the user stores their own key, and only the user
    // can do that — so it gets its own reason and a button to Settings.
    if !models.missing_personal_keys.is_empty() {
        return Some(Finding::new(
            AgentState::Attention,
            AgentReason::PersonalKeyMissing {
                providers: models.missing_personal_keys.join(", "),
            },
            Some(AgentAction::AddPersonalKey),
        ));
    }
    if models.available {
        return None;
    }
    let reason = if models.unconfigured_providers.is_empty() {
        AgentReason::NoModels
    } else {
        AgentReason::NoKey {
            providers: models.unconfigured_providers.join(", "),
        }
    };
    Some(Finding::new(AgentState::Attention, reason, None))
}

fn restart_finding(input: &HostHealthInputs<'_>, is_running: bool) -> Option<Finding> {
    (input.pending == PendingStep::Restart && is_running).then(|| {
        Finding::new(
            AgentState::RestartNeeded,
            AgentReason::RestartPending,
            Some(AgentAction::Restart),
        )
    })
}

fn proxy_finding(
    input: &HostHealthInputs<'_>,
    snap: &crate::integration::HostAppSnapshot,
) -> Finding {
    let open = input.can_open.then_some(AgentAction::Open);
    match input.proxy.state {
        ProxyProbeState::Unconfigured => {
            Finding::new(AgentState::Ready, AgentReason::Awaiting, open)
        },
        ProxyProbeState::Listening => Finding::new(
            AgentState::Working,
            AgentReason::Governed {
                when_unix: (snap.probed_at_unix > 0).then_some(snap.probed_at_unix),
            },
            open,
        ),
        ProxyProbeState::Unknown => {
            Finding::new(AgentState::Checking, AgentReason::NeverProbed, None)
        },
        probe => Finding::new(
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

fn profile_finding(snap: &crate::integration::HostAppSnapshot) -> Option<Finding> {
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
            Some(Finding::new(
                state,
                AgentReason::Stale {
                    cause: *reason,
                    keys: keys.join(", "),
                },
                Some(action),
            ))
        },
        ProfileState::Partial { missing_required } => Some(Finding::new(
            AgentState::Attention,
            AgentReason::Partial {
                missing: missing_required.join(", "),
            },
            Some(AgentAction::Repair),
        )),
        ProfileState::Unverifiable { reason } => Some(Finding::new(
            AgentState::Attention,
            AgentReason::Unverifiable {
                detail: reason.clone(),
            },
            Some(AgentAction::Verify),
        )),
        ProfileState::Absent => Some(Finding::new(
            AgentState::NotSetUp,
            AgentReason::Absent,
            Some(AgentAction::Connect),
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
pub use super::sync_only::{SYNC_ONLY_AGENTS, SyncOnlyAgent, sync_only_agent, sync_only_agents};
