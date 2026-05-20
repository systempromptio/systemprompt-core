//! Bootstrap (install_from_governance_config) tests.
//!
//! Verifies the fail-closed contract: every untrusted config path must end at
//! `DenyAllHook` or a bootstrap error. Allow-all is reachable only via the
//! literal acknowledgement string.

use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_models::profile::{
    AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT,
};
use systemprompt_security::authz::{
    AuthzBootstrapError, AuthzDecision, AuthzError, AuthzRequest, EntityKind, clear_global_hook,
    global_hook, install_from_governance_config,
};
use tokio::sync::{Mutex, MutexGuard};

// Why: install_from_governance_config mutates a process-global hook slot, so
// these tests cannot interleave. The lock is held across `.await` points
// (hook.evaluate), so it must be a tokio Mutex — a std Mutex held across
// await triggers clippy::await_holding_lock and is genuinely unsafe in a
// multi-threaded runtime.
static SERIAL: Mutex<()> = Mutex::const_new(());

async fn serial_guard() -> MutexGuard<'static, ()> {
    SERIAL.lock().await
}

fn fixture() -> AuthzRequest {
    AuthzRequest {
        entity_type: EntityKind::GatewayRoute,
        entity_id: "claude-3".into(),
        user_id: UserId::new("u1"),
        roles: vec!["eng".into()],
        department: "platform".into(),
        trace_id: TraceId::new("trace-1"),
        context: serde_json::Value::Null,
    }
}

fn governance_with(mode: AuthzMode, url: Option<&str>, ack: Option<&str>) -> GovernanceConfig {
    GovernanceConfig {
        authz: Some(AuthzConfig {
            hook: AuthzHookConfig {
                mode,
                url: url.map(str::to_owned),
                timeout_ms: 100,
                acknowledgement: ack.map(str::to_owned),
            },
        }),
    }
}

#[tokio::test]
async fn no_governance_block_installs_deny_all() {
    let _serial = serial_guard().await;
    clear_global_hook();
    install_from_governance_config(None, None).expect("install ok");
    let hook = global_hook().expect("hook installed");
    let decision = hook.evaluate(fixture()).await;
    assert!(
        matches!(decision, AuthzDecision::Deny { .. }),
        "absent governance must install DenyAllHook (got {:?})",
        decision
    );
}

#[tokio::test]
async fn disabled_mode_installs_deny_all() {
    let _serial = serial_guard().await;
    clear_global_hook();
    let cfg = governance_with(AuthzMode::Disabled, None, None);
    install_from_governance_config(Some(&cfg), None).expect("install ok");
    let hook = global_hook().expect("hook installed");
    let decision = hook.evaluate(fixture()).await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}

#[tokio::test]
async fn webhook_mode_without_url_errors() {
    let _serial = serial_guard().await;
    clear_global_hook();
    let cfg = governance_with(AuthzMode::Webhook, None, None);
    let err = install_from_governance_config(Some(&cfg), None)
        .expect_err("missing url must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingWebhookUrl)
    ));
}

#[tokio::test]
async fn webhook_mode_with_blank_url_errors() {
    let _serial = serial_guard().await;
    clear_global_hook();
    let cfg = governance_with(AuthzMode::Webhook, Some("   "), None);
    let err = install_from_governance_config(Some(&cfg), None)
        .expect_err("blank url must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingWebhookUrl)
    ));
}

#[tokio::test]
async fn unrestricted_without_acknowledgement_errors() {
    let _serial = serial_guard().await;
    clear_global_hook();
    let cfg = governance_with(AuthzMode::Unrestricted, None, None);
    let err = install_from_governance_config(Some(&cfg), None)
        .expect_err("missing acknowledgement must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingUnrestrictedAcknowledgement { .. })
    ));
}

#[tokio::test]
async fn unrestricted_with_wrong_acknowledgement_errors() {
    let _serial = serial_guard().await;
    clear_global_hook();
    let cfg = governance_with(AuthzMode::Unrestricted, None, Some("yolo"));
    let err = install_from_governance_config(Some(&cfg), None)
        .expect_err("wrong acknowledgement must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingUnrestrictedAcknowledgement { .. })
    ));
}

#[tokio::test]
async fn unrestricted_with_correct_acknowledgement_installs_allow_all() {
    let _serial = serial_guard().await;
    clear_global_hook();
    let cfg = governance_with(
        AuthzMode::Unrestricted,
        None,
        Some(UNRESTRICTED_ACKNOWLEDGEMENT),
    );
    install_from_governance_config(Some(&cfg), None).expect("install ok with acknowledgement");
    let hook = global_hook().expect("hook installed");
    let decision = hook.evaluate(fixture()).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn webhook_mode_with_url_installs_webhook_hook() {
    let _serial = serial_guard().await;
    clear_global_hook();
    let cfg = governance_with(AuthzMode::Webhook, Some("http://127.0.0.1:1/authz"), None);
    install_from_governance_config(Some(&cfg), None).expect("install ok");
    let hook = global_hook().expect("hook installed");
    let decision = hook.evaluate(fixture()).await;
    match &decision {
        AuthzDecision::Deny { reason, policy } => {
            assert_eq!(reason, "authz hook unreachable");
            assert_eq!(policy, "authz_hook_fault");
        },
        AuthzDecision::Allow => panic!("unreachable webhook must deny, got Allow"),
    }
}
