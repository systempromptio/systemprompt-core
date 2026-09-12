use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::gui::state::{
    AppState, AppStateSnapshot, GatewayProbeOutcome, GatewayStatus, VerifiedIdentity,
};
use systemprompt_bridge::obs::StartupFault;
use systemprompt_bridge::verdict::Tone;
use systemprompt_bridge::wire::DeviceAction;
use systemprompt_bridge::wire::codes::{HealthCode, IdentityCode};

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
        status: GatewayStatus::Reachable { latency_ms: 4 },
        identity: None,
        at_unix: 600,
        provider_health: Vec::new(),
        credential_error: Some("authentication: token rejected".to_owned()),
    });
    state.apply_probe(GatewayProbeOutcome {
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
