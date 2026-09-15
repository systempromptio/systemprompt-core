use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::gui::state::{
    AppState, AppStateSnapshot, GatewayProbeOutcome, GatewayStatus, VerifiedIdentity,
};
use systemprompt_bridge::obs::StartupFault;
use systemprompt_bridge::verdict::Tone;
use systemprompt_bridge::wire::DeviceAction;
use systemprompt_bridge::ids::HostId;
use systemprompt_bridge::sync::{HostFailure, SyncSummary};
use systemprompt_bridge::wire::codes::{HealthCode, IdentityCode, OverallCode};

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
        provider_health: Vec::new(),
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
        provider_health: Vec::new(),
        credential_error: Some("authentication: token rejected".to_owned()),
    });
    state.apply_probe(GatewayProbeOutcome {
        gateway: systemprompt_identifiers::ValidatedUrl::try_new("https://gateway.example.com")
            .expect("valid gateway url"),
        status: GatewayStatus::Reachable { latency_ms: 4 },
        identity: Some(identity()),
        at_unix: 660,
        provider_health: Vec::new(),
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

#[test]
fn a_clean_sync_report_reads_as_synced() {
    let mut snap = signed_in_after_sync();
    snap.last_sync_report = Some(report(vec![]));
    let verdict = snap.overall_verdict();
    assert_eq!(verdict.code, OverallCode::Synced);
    assert_eq!(verdict.tone, Tone::Ok);
    assert!(!snap.last_sync_degraded());
}

// Why: the header pill said "synced" beside a toast naming a host that had
// failed; the run's own report, not the presence of a summary line, decides.
#[test]
fn a_host_failure_in_the_last_report_reads_as_degraded_even_with_a_summary() {
    let mut snap = signed_in_after_sync();
    snap.last_sync_report = Some(report(vec![HostFailure {
        host_id: HostId::new("claude-desktop"),
        emitter: "claude-desktop".to_owned(),
        error: "mdm refresh: HKLM shadows HKCU".into(),
        needs_elevation: true,
    }]));
    let verdict = snap.overall_verdict();
    assert_eq!(verdict.code, OverallCode::Degraded);
    assert_eq!(verdict.tone, Tone::Warn);
    assert!(snap.last_sync_degraded());
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
        host_id: HostId::new("codex-cli"),
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
