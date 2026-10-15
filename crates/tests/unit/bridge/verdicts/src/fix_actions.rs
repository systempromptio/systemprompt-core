//! One fix per agent state: every installed agent app is listed, and each
//! state the Agents tab can show carries the one button that fixes it.
//!
//! The macOS case that motivated this: Claude Desktop installed, its
//! `.mobileconfig` waiting in System Settings › Device Management, so the probe
//! read no profile — and the list, which showed only set-up agents, hid it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_bridge::integration::agent_health::{
    AgentAction, AgentFleets, AgentReason, AgentState, AgentSurface, AgentVerdict, ApplyFailure,
    HostHealthInputs, HostModelViewRef, OS_APPROVAL_WINDOW_SECS, PendingStep, verdict,
};
use systemprompt_bridge::integration::host_app::{
    AppInstallState, HostAppSnapshot, ProfileState, StaleReason,
};
use systemprompt_bridge::proxy_probe::{ProxyHealth, ProxyProbeState};
use systemprompt_bridge::verdict::Tone;
use systemprompt_models::bridge::host::HostKind;

fn snapshot(profile_state: ProfileState, app: AppInstallState, running: bool) -> HostAppSnapshot {
    HostAppSnapshot {
        host_id: HostKind::ClaudeDesktop,
        display_name: "Claude Desktop",
        profile_state,
        profile_source: None,
        profile_keys: BTreeMap::new(),
        probe_error: None,
        host_running: Some(running),
        host_processes: Vec::new(),
        app_installed: app,
        probed_at_unix: 1_700_000_000,
        update_needs_approval: false,
    }
}

// Offered one minute before the probe in `snapshot`, so inside the window.
const OFFERED: PendingStep = PendingStep::OsApproval {
    offered_at_unix: 1_700_000_000 - 60,
};

fn listening() -> ProxyHealth {
    ProxyHealth {
        state: ProxyProbeState::Listening,
        ..Default::default()
    }
}

fn judge(snap: &HostAppSnapshot, pending: PendingStep, failures: &[ApplyFailure]) -> AgentVerdict {
    let proxy = listening();
    verdict(&HostHealthInputs {
        snapshot: Some(snap),
        apply_failures: failures,
        proxy: &proxy,
        models: HostModelViewRef {
            checked: true,
            available: true,
            unconfigured_providers: &[],
            compatible_models: &[],
        },
        has_download_url: true,
        surface: AgentSurface::LocalProfile,
        manifest_synced: true,
        gateway_routed: true,
        can_open: true,
        pending,
    })
}

#[test]
fn a_profile_awaiting_macos_approval_is_listed_and_opens_device_management() {
    let snap = snapshot(ProfileState::Absent, AppInstallState::Installed, false);
    let v = judge(&snap, OFFERED, &[]);
    assert_eq!(v.state, AgentState::NeedsApproval);
    assert_eq!(v.tone, Tone::Warn);
    assert!(matches!(v.reason, AgentReason::AwaitingOsApproval));
    assert_eq!(v.action, Some(AgentAction::OpenDeviceManagement));
    assert!(v.is_listed, "the approval-pending app must be visible");
    assert!(!v.is_set_up);
}

#[test]
fn a_profile_left_unapproved_past_the_macos_window_must_be_offered_again() {
    let mut snap = snapshot(ProfileState::Absent, AppInstallState::Installed, false);
    snap.probed_at_unix = 1_700_000_000 - 60 + OS_APPROVAL_WINDOW_SECS;
    let v = judge(&snap, OFFERED, &[]);
    assert_eq!(v.state, AgentState::ApprovalExpired);
    assert!(matches!(v.reason, AgentReason::ApprovalExpired));
    assert_eq!(v.action, Some(AgentAction::OfferAgain));
    assert_eq!(v.secondary_action, Some(AgentAction::ShowProfileFile));
    assert!(v.is_listed);

    snap.probed_at_unix -= 1;
    assert_eq!(
        judge(&snap, OFFERED, &[]).state,
        AgentState::NeedsApproval,
        "one second inside the window is still awaiting approval"
    );
}

#[test]
fn an_installed_app_with_no_profile_is_listed_with_connect() {
    let snap = snapshot(ProfileState::Absent, AppInstallState::Installed, false);
    let v = judge(&snap, PendingStep::None, &[]);
    assert_eq!(v.state, AgentState::NotSetUp);
    assert_eq!(v.action, Some(AgentAction::Connect));
    assert!(v.is_listed);
}

#[test]
fn a_stale_profile_offers_reapply_and_an_elevated_one_admin_approval() {
    let stale = ProfileState::Stale {
        reason: StaleReason::LoopbackSecret,
        keys: vec!["inferenceGatewayApiKey".to_owned()],
    };
    let snap = snapshot(stale, AppInstallState::Installed, false);
    let v = judge(&snap, PendingStep::None, &[]);
    assert_eq!(v.state, AgentState::NeedsReapply);
    assert_eq!(v.action, Some(AgentAction::Repair));

    let mut drift = snapshot(
        ProfileState::Stale {
            reason: StaleReason::Settings,
            keys: vec!["otlpEndpoint".to_owned()],
        },
        AppInstallState::Installed,
        false,
    );
    drift.update_needs_approval = true;
    let v = judge(&drift, PendingStep::None, &[]);
    assert_eq!(v.state, AgentState::NeedsAdminApproval);
    assert_eq!(v.action, Some(AgentAction::UpdateAdmin));
}

#[test]
fn an_elevation_only_sync_failure_needs_admin_approval() {
    let snap = snapshot(ProfileState::Installed, AppInstallState::Installed, false);
    let failures = [ApplyFailure {
        emitter: "claude-desktop".to_owned(),
        error: "access denied".to_owned(),
        needs_elevation: true,
    }];
    let v = judge(&snap, PendingStep::None, &failures);
    assert_eq!(v.state, AgentState::NeedsAdminApproval);
    assert_eq!(v.action, Some(AgentAction::UpdateAdmin));
}

#[test]
fn a_profile_written_while_the_app_ran_asks_for_a_restart() {
    let open = snapshot(ProfileState::Installed, AppInstallState::Installed, true);
    let v = judge(&open, PendingStep::Restart, &[]);
    assert_eq!(v.state, AgentState::RestartNeeded);
    assert!(matches!(v.reason, AgentReason::RestartPending));
    assert_eq!(v.action, Some(AgentAction::Restart));

    let closed = snapshot(ProfileState::Installed, AppInstallState::Installed, false);
    assert_eq!(
        judge(&closed, PendingStep::Restart, &[]).state,
        AgentState::Working,
        "a closed app reads the new profile on its next launch"
    );
}

#[test]
fn a_missing_app_with_leftover_settings_offers_download_and_removal() {
    let partial = ProfileState::Partial {
        missing_required: vec!["model".to_owned()],
    };
    let snap = snapshot(partial, AppInstallState::NotInstalled, false);
    let v = judge(&snap, PendingStep::None, &[]);
    assert_eq!(v.state, AgentState::NotInstalled);
    assert!(matches!(v.reason, AgentReason::LeftoverConfig));
    assert_eq!(v.action, Some(AgentAction::Download));
    assert_eq!(v.secondary_action, Some(AgentAction::RemoveLeftover));
    assert!(v.is_listed);
}

#[test]
fn only_an_absent_app_with_nothing_left_over_is_hidden() {
    let snap = snapshot(ProfileState::Absent, AppInstallState::NotInstalled, false);
    let v = judge(&snap, PendingStep::None, &[]);
    assert_eq!(v.state, AgentState::NotInstalled);
    assert!(matches!(v.reason, AgentReason::AppMissing));
    assert_eq!(v.secondary_action, None);
    assert!(!v.is_listed);
}

#[test]
fn the_listed_fleet_counts_an_approval_as_attention() {
    let pending = snapshot(ProfileState::Absent, AppInstallState::Installed, false);
    let fleets = AgentFleets::fold(&[judge(&pending, OFFERED, &[])]);
    assert_eq!(
        fleets.set_up.total, 1,
        "the header no longer says none installed"
    );
    assert_eq!(fleets.set_up.attention, 1);
}

#[test]
fn a_probe_moves_the_pending_step_forward() {
    let absent = snapshot(ProfileState::Absent, AppInstallState::Installed, true);
    let installed_open = snapshot(ProfileState::Installed, AppInstallState::Installed, true);
    let installed_closed = snapshot(ProfileState::Installed, AppInstallState::Installed, false);

    assert_eq!(OFFERED.after_probe(&absent), OFFERED);
    assert_eq!(
        OFFERED.after_probe(&installed_open),
        PendingStep::Restart,
        "an approved profile still needs the open app restarted"
    );
    assert_eq!(OFFERED.after_probe(&installed_closed), PendingStep::None);
    assert_eq!(
        PendingStep::Restart.after_probe(&installed_closed),
        PendingStep::None
    );

    let mut unknown = installed_open.clone();
    unknown.host_running = None;
    assert_eq!(
        PendingStep::Restart.after_probe(&unknown),
        PendingStep::Restart,
        "an unreadable process table is not proof the app restarted"
    );
}
