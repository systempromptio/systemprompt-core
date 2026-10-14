use std::collections::BTreeMap;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::gui::hosts::state::HostState;
use systemprompt_bridge::gui::server_json::state_payload;
use systemprompt_bridge::gui::state::{
    AppState, AppStateSnapshot, GatewayProbeOutcome, GatewayStatus, VerifiedIdentity,
};
use systemprompt_bridge::integration::{
    AppInstallState, HostAppSnapshot, ProfileState, StaleReason,
};
use systemprompt_bridge::obs::StartupFault;
use systemprompt_bridge::sync::{HostFailure, SyncSummary};
use systemprompt_bridge::verdict::Tone;
use systemprompt_bridge::wire::DeviceAction;
use systemprompt_bridge::wire::codes::{HealthCode, IdentityCode, OverallCode};
use systemprompt_models::bridge::host::HostKind;

fn reachable() -> AppStateSnapshot {
    AppStateSnapshot {
        gateway_status: GatewayStatus::Reachable { latency_ms: 4 },
        ..AppStateSnapshot::default()
    }
}

#[test]
fn pending_device_action_survives_a_state_reload_until_dismissed() {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    let state = AppState::new_loaded(ctx);
    state.set_pending_device_action(Some(DeviceAction::Purge));
    state.reload();
    assert_eq!(
        state.snapshot().pending_device_action,
        Some(DeviceAction::Purge)
    );
    state.set_pending_device_action(None);
    assert_eq!(state.snapshot().pending_device_action, None);
}

fn identity() -> VerifiedIdentity {
    VerifiedIdentity {
        email: Some("user@example.com".to_owned()),
        user_id: None,
        tenant_id: None,
        exp_unix: None,
        verified_at_unix: 600,
    }
}

#[test]
fn a_reachable_gateway_that_rejected_the_credential_is_a_rejected_token_not_a_signed_out_user() {
    let mut snap = reachable();
    snap.credential_error = Some("authentication: token rejected".to_owned());

    let verdict = snap.identity_verdict();
    assert_eq!(verdict.code, IdentityCode::TokenRejected);
    assert_eq!(
        verdict.tone,
        Tone::Err,
        "a rejected credential is an error the operator must act on"
    );
}

#[test]
fn a_credential_error_never_overrides_an_identity_the_gateway_verified() {
    let mut snap = reachable();
    snap.verified_identity = Some(identity());
    snap.credential_error = Some("provider health: unreachable".to_owned());

    let verdict = snap.identity_verdict();
    assert_eq!(verdict.code, IdentityCode::SignedIn);
    assert_eq!(verdict.tone, Tone::Ok);
}

#[test]
fn without_a_credential_error_a_reachable_gateway_and_no_identity_is_merely_signed_out() {
    let verdict = reachable().identity_verdict();
    assert_eq!(
        verdict.code,
        IdentityCode::SignedOut,
        "the TokenRejected verdict must come from the error, not from the gateway state alone"
    );
    assert_eq!(verdict.tone, Tone::Warn);
}

#[test]
fn a_startup_fault_alone_makes_health_failing() {
    let mut snap = AppStateSnapshot::default();
    assert_eq!(
        snap.health_verdict().code,
        HealthCode::NotChecked,
        "nothing observed yet is not a health claim"
    );

    snap.startup_faults = vec![StartupFault::new(
        "proxy port file",
        "parse: expected value",
    )];
    let verdict = snap.health_verdict();
    assert_eq!(verdict.tone, Tone::Err);
    assert_eq!(verdict.code, HealthCode::Failing);
}

#[test]
fn a_credential_error_alone_makes_health_failing() {
    let mut snap = AppStateSnapshot::default();
    snap.credential_error = Some("authentication: token rejected".to_owned());

    let verdict = snap.health_verdict();
    assert_eq!(verdict.tone, Tone::Err);
    assert_eq!(verdict.code, HealthCode::Failing);
}

#[test]
fn applying_a_probe_carries_the_credential_error_into_the_snapshot() {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    let state = AppState::new_loaded(ctx);

    state.apply_probe(GatewayProbeOutcome {
        gateway: systemprompt_identifiers::ValidatedUrl::try_new("https://gateway.example.com")
            .expect("valid gateway url"),
        status: GatewayStatus::Reachable { latency_ms: 4 },
        identity: None,
        at_unix: 600,
        catalog: Ok(Vec::new()),
        credential_error: Some("authentication: token rejected".to_owned()),
    });

    let snap = state.snapshot();
    assert_eq!(
        snap.credential_error.as_deref(),
        Some("authentication: token rejected"),
        "the probe's reason for rejecting the credential is what the GUI renders"
    );
    assert_eq!(snap.identity_verdict().code, IdentityCode::TokenRejected);
    assert_eq!(snap.health_verdict().code, HealthCode::Failing);
}

#[test]
fn a_later_clean_probe_clears_the_credential_error() {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    let state = AppState::new_loaded(ctx);

    state.apply_probe(GatewayProbeOutcome {
        gateway: systemprompt_identifiers::ValidatedUrl::try_new("https://gateway.example.com")
            .expect("valid gateway url"),
        status: GatewayStatus::Reachable { latency_ms: 4 },
        identity: None,
        at_unix: 600,
        catalog: Ok(Vec::new()),
        credential_error: Some("authentication: token rejected".to_owned()),
    });
    state.apply_probe(GatewayProbeOutcome {
        gateway: systemprompt_identifiers::ValidatedUrl::try_new("https://gateway.example.com")
            .expect("valid gateway url"),
        status: GatewayStatus::Reachable { latency_ms: 4 },
        identity: Some(identity()),
        at_unix: 660,
        catalog: Ok(Vec::new()),
        credential_error: None,
    });

    let snap = state.snapshot();
    assert!(
        snap.credential_error.is_none(),
        "a stale rejection must not outlive the probe that resolved it"
    );
    assert_eq!(snap.identity_verdict().code, IdentityCode::SignedIn);
}

fn signed_in_after_sync() -> AppStateSnapshot {
    let mut snap = reachable();
    snap.verified_identity = Some(identity());
    snap.last_sync_summary = Some("sync ok (user@example.com)".to_owned());
    snap
}

fn report(host_failures: Vec<HostFailure>) -> SyncSummary {
    SyncSummary {
        identity: "user@example.com".into(),
        manifest_version: "2026-09-15T00:00:00Z-deadbeef".into(),
        plugin_count: 3,
        skill_count: 8,
        rule_count: 0,
        agent_count: 0,
        hook_count: 0,
        mcp_count: 2,
        artifact_count: 1,
        installed: vec![],
        updated: vec!["a".into(), "b".into(), "c".into()],
        removed: vec![],
        malformed: vec![],
        host_failures,
        host_warnings: Vec::new(),
        diagnostics: vec![],
    }
}

fn host(id: HostKind, profile_state: ProfileState, update_needs_approval: bool) -> HostState {
    HostState {
        snapshot: Some(HostAppSnapshot {
            host_id: id,
            display_name: "host",
            profile_state,
            profile_source: None,
            profile_keys: BTreeMap::new(),
            probe_error: None,
            host_running: Some(true),
            host_processes: Vec::new(),
            app_installed: AppInstallState::Installed,
            probed_at_unix: 1,
            update_needs_approval,
        }),
        ..HostState::default()
    }
}

fn with_hosts(mut snap: AppStateSnapshot, hosts: Vec<(HostKind, HostState)>) -> AppStateSnapshot {
    snap.enabled_hosts = hosts.iter().map(|(id, _)| id.as_str().to_owned()).collect();
    snap.hosts.by_id.extend(hosts);
    snap
}

fn failure(id: HostKind, emitter: &str, needs_elevation: bool) -> HostFailure {
    HostFailure {
        host_id: id,
        emitter: emitter.to_owned(),
        error: "write refused".into(),
        needs_elevation,
    }
}

fn desktop_row(snap: &AppStateSnapshot) -> serde_json::Value {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    let payload = serde_json::to_value(state_payload(snap, &ctx.proxy)).expect("payload");
    payload["host_apps"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == "claude-desktop"))
        .map(|r| r["verdict"].clone())
        .expect("desktop is listed")
}

#[test]
fn a_clean_sync_report_reads_as_synced() {
    let mut snap = signed_in_after_sync();
    snap.last_sync_report = Some(report(vec![]));
    let verdict = snap.overall_verdict();
    assert_eq!(verdict.code, OverallCode::Synced);
    assert_eq!(verdict.tone, Tone::Ok);
}

// Why: the v0.65.0 header said Claude Desktop needed approval while its row
// said Working. Both now read the same verdict: approval is needed exactly
// when some row's action is the administrator update.
#[test]
fn needs_approval_is_exactly_a_row_whose_action_is_the_administrator_update() {
    let drifted = ProfileState::Stale {
        reason: StaleReason::Settings,
        keys: vec!["otlpEndpoint".to_owned()],
    };
    let snap = with_hosts(
        signed_in_after_sync(),
        vec![(
            HostKind::ClaudeDesktop,
            host(HostKind::ClaudeDesktop, drifted.clone(), true),
        )],
    );
    assert_eq!(desktop_row(&snap)["action"]["code"], "update-admin");
    let verdict = snap.overall_verdict();
    assert_eq!(verdict.code, OverallCode::NeedsApproval);
    assert_eq!(verdict.tone, Tone::Warn);

    let plain = with_hosts(
        signed_in_after_sync(),
        vec![(
            HostKind::ClaudeDesktop,
            host(HostKind::ClaudeDesktop, drifted, false),
        )],
    );
    assert_eq!(desktop_row(&plain)["action"]["code"], "update");
    assert_ne!(plain.overall_verdict().code, OverallCode::NeedsApproval);
}

// Why: a machine-policy write that needed an administrator is history once
// the live probe reads the policy current — a later repair wrote it — so the
// header follows the probe, not the old run.
#[test]
fn an_approval_failure_the_probe_now_reads_as_current_is_superseded() {
    let mut snap = with_hosts(
        signed_in_after_sync(),
        vec![(
            HostKind::ClaudeDesktop,
            host(HostKind::ClaudeDesktop, ProfileState::Installed, true),
        )],
    );
    snap.last_sync_report = Some(report(vec![failure(
        HostKind::ClaudeDesktop,
        "claude-desktop",
        true,
    )]));
    let row = desktop_row(&snap);
    assert_ne!(row["reason"]["code"], "sync-failed", "{row}");
    assert_eq!(snap.overall_verdict().code, OverallCode::Synced);
}

#[test]
fn a_failed_step_the_probe_cannot_see_shows_on_the_row_and_degrades_the_header() {
    let mut snap = with_hosts(
        signed_in_after_sync(),
        vec![(
            HostKind::ClaudeDesktop,
            host(HostKind::ClaudeDesktop, ProfileState::Installed, false),
        )],
    );
    snap.last_sync_report = Some(report(vec![failure(
        HostKind::ClaudeDesktop,
        "cowork-plugins",
        false,
    )]));
    let row = desktop_row(&snap);
    assert_eq!(row["state"], "attention", "{row}");
    assert_eq!(row["reason"]["code"], "sync-failed", "{row}");
    let verdict = snap.overall_verdict();
    assert_eq!(verdict.code, OverallCode::Degraded);
    assert_eq!(verdict.tone, Tone::Warn);
}

#[test]
fn a_failure_on_a_host_that_is_not_listed_still_degrades_the_header() {
    let mut snap = with_hosts(
        signed_in_after_sync(),
        vec![(
            HostKind::ClaudeDesktop,
            host(HostKind::ClaudeDesktop, ProfileState::Installed, false),
        )],
    );
    snap.last_sync_report = Some(report(vec![failure(HostKind::OpenCode, "opencode", false)]));
    assert_eq!(snap.overall_verdict().code, OverallCode::Degraded);
}

#[test]
fn a_partial_run_that_left_no_summary_line_is_still_degraded_not_ready() {
    let mut snap = signed_in_after_sync();
    snap.last_sync_summary = None;
    let mut summary = report(vec![]);
    summary.malformed = vec!["broken-plugin".into()];
    snap.last_sync_report = Some(summary);
    assert_eq!(snap.overall_verdict().code, OverallCode::Degraded);
}

#[test]
fn syncing_and_offline_outrank_a_degraded_report() {
    let mut snap = signed_in_after_sync();
    snap.last_sync_report = Some(report(vec![HostFailure {
        host_id: HostKind::CodexCli,
        emitter: "codex-cli".to_owned(),
        error: "permission denied".into(),
        needs_elevation: false,
    }]));
    snap.sync_in_flight = true;
    assert_eq!(snap.overall_verdict().code, OverallCode::Syncing);
    snap.sync_in_flight = false;
    snap.gateway_status = GatewayStatus::Unreachable {
        reason: "timeout".into(),
    };
    assert_eq!(snap.overall_verdict().code, OverallCode::Offline);
}
