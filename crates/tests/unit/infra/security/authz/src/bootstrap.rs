//! `build_authz_hook` tests.
//!
//! Verifies the fail-closed contract: every untrusted config path must end at
//! `DenyAllHook` or a bootstrap error. Allow-all is reachable only via the
//! literal acknowledgement string.

use systemprompt_identifiers::{RouteId, TraceId};
use systemprompt_models::profile::{
    AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT,
};
use systemprompt_security::authz::{
    AuthzBootstrapError, AuthzContext, AuthzDecision, AuthzError, AuthzRequest, DenyReason,
    EntityRef, build_authz_hook,
};
use systemprompt_test_fixtures::fixture_user_id;

fn fixture() -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("claude-3")),
        user_id: fixture_user_id(),
        roles: vec!["eng".into()],
        department: "platform".into(),
        trace_id: TraceId::new("trace-1"),
        context: AuthzContext::None,
        act_chain: Vec::new(),
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
async fn no_governance_block_yields_deny_all() {
    let hook = build_authz_hook(None, None).expect("build ok");
    let decision = hook.evaluate(fixture()).await;
    assert!(
        matches!(decision, AuthzDecision::Deny { .. }),
        "absent governance must yield DenyAllHook (got {decision:?})",
    );
}

#[tokio::test]
async fn disabled_mode_yields_deny_all() {
    let cfg = governance_with(AuthzMode::Disabled, None, None);
    let hook = build_authz_hook(Some(&cfg), None).expect("build ok");
    let decision = hook.evaluate(fixture()).await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}

#[tokio::test]
async fn webhook_mode_without_url_errors() {
    let cfg = governance_with(AuthzMode::Webhook, None, None);
    let err = build_authz_hook(Some(&cfg), None).expect_err("missing url must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingWebhookUrl)
    ));
}

#[tokio::test]
async fn webhook_mode_with_blank_url_errors() {
    let cfg = governance_with(AuthzMode::Webhook, Some("   "), None);
    let err = build_authz_hook(Some(&cfg), None).expect_err("blank url must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingWebhookUrl)
    ));
}

#[tokio::test]
async fn webhook_mode_with_metadata_ip_url_errors() {
    let cfg = governance_with(
        AuthzMode::Webhook,
        Some("http://169.254.169.254/authz"),
        None,
    );
    let err =
        build_authz_hook(Some(&cfg), None).expect_err("cloud-metadata url must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::InvalidWebhookUrl(_))
    ));
}

#[tokio::test]
async fn webhook_mode_with_private_range_url_errors() {
    let cfg = governance_with(AuthzMode::Webhook, Some("https://10.0.0.5/authz"), None);
    let err =
        build_authz_hook(Some(&cfg), None).expect_err("private-range url must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::InvalidWebhookUrl(_))
    ));
}

#[tokio::test]
async fn webhook_mode_with_non_loopback_http_url_errors() {
    let cfg = governance_with(AuthzMode::Webhook, Some("http://authz.example.com/h"), None);
    let err =
        build_authz_hook(Some(&cfg), None).expect_err("non-loopback http url must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::InvalidWebhookUrl(_))
    ));
}

#[tokio::test]
async fn unrestricted_without_acknowledgement_errors() {
    let cfg = governance_with(AuthzMode::Unrestricted, None, None);
    let err = build_authz_hook(Some(&cfg), None)
        .expect_err("missing acknowledgement must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingUnrestrictedAcknowledgement { .. })
    ));
}

#[tokio::test]
async fn unrestricted_with_wrong_acknowledgement_errors() {
    let cfg = governance_with(AuthzMode::Unrestricted, None, Some("yolo"));
    let err =
        build_authz_hook(Some(&cfg), None).expect_err("wrong acknowledgement must fail bootstrap");
    assert!(matches!(
        err,
        AuthzError::Bootstrap(AuthzBootstrapError::MissingUnrestrictedAcknowledgement { .. })
    ));
}

#[tokio::test]
async fn unrestricted_with_correct_acknowledgement_yields_allow_all() {
    let cfg = governance_with(
        AuthzMode::Unrestricted,
        None,
        Some(UNRESTRICTED_ACKNOWLEDGEMENT),
    );
    let hook = build_authz_hook(Some(&cfg), None).expect("build ok with acknowledgement");
    let decision = hook.evaluate(fixture()).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn webhook_mode_with_url_yields_webhook_hook() {
    let cfg = governance_with(AuthzMode::Webhook, Some("http://127.0.0.1:1/authz"), None);
    let hook = build_authz_hook(Some(&cfg), None).expect("build ok");
    let decision = hook.evaluate(fixture()).await;
    match &decision {
        AuthzDecision::Deny { reason, policy } => {
            assert_eq!(policy, "authz_hook_fault");
            assert!(matches!(
                reason,
                DenyReason::HookUnavailable { policy: p } if p == "authz_hook_fault"
            ));
        },
        AuthzDecision::Allow => panic!("unreachable webhook must deny, got Allow"),
    }
}
