//! The gateway request-guard seam: registration, request propagation, and
//! deny-kind semantics.
//!
//! `inventory` registration is binary-wide, so the guard registered here is
//! armed per test via a static and left inert otherwise.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use systemprompt_extension::{
    GatewayDenyKind, GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard, gateway_guards,
    register_gateway_guard, run_gateway_guards,
};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, UserId};
use systemprompt_traits::DatabaseHandle;

static ARMED: AtomicBool = AtomicBool::new(false);
// user, model, route, provider, streaming — as observed by the guard.
type SeenRequest = (String, String, Option<String>, String, bool);

static SEEN: Mutex<Option<SeenRequest>> = Mutex::new(None);

#[derive(Default)]
struct RecordingGuard;

#[async_trait::async_trait]
impl GatewayRequestGuard for RecordingGuard {
    async fn check(
        &self,
        db: &dyn DatabaseHandle,
        request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        if !ARMED.load(Ordering::SeqCst) {
            return Ok(());
        }
        assert!(
            db.as_any().downcast_ref::<StubDb>().is_some(),
            "guard receives the concrete handle it was compiled against"
        );
        *SEEN.lock().expect("seen lock") = Some((
            request.user_id.to_string(),
            request.model.to_string(),
            request.route_id.map(ToString::to_string),
            request.provider.to_string(),
            request.streaming,
        ));
        Err(GatewayDenyReason::forbidden(
            "your plan does not include this model",
        ))
    }
}

register_gateway_guard!(RecordingGuard);

struct StubDb;

impl DatabaseHandle for StubDb {
    fn is_connected(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test]
async fn an_unarmed_registry_admits_the_request() {
    let user = UserId::new("user_1");
    let model = ModelId::new("claude-opus-5");
    let route = RouteId::new("route_default");
    let provider = ProviderId::new("anthropic");
    let request = GatewayGuardRequest {
        user_id: &user,
        model: &model,
        route_id: Some(&route),
        provider: &provider,
        streaming: false,
    };
    assert!(run_gateway_guards(&StubDb, &request).await.is_ok());
}

#[test]
fn guards_are_materialised_once_from_inventory() {
    let first = gateway_guards();
    let second = gateway_guards();
    assert!(!first.is_empty());
    assert!(std::ptr::eq(first.as_ptr(), second.as_ptr()));
}

#[tokio::test]
async fn a_guard_sees_the_resolved_request_and_its_forbidden_kind_survives() {
    ARMED.store(true, Ordering::SeqCst);
    let user = UserId::new("user_2");
    let model = ModelId::new("claude-opus-5");
    let route = RouteId::new("route_premium");
    let provider = ProviderId::new("anthropic");
    let request = GatewayGuardRequest {
        user_id: &user,
        model: &model,
        route_id: Some(&route),
        provider: &provider,
        streaming: true,
    };
    let deny = run_gateway_guards(&StubDb, &request)
        .await
        .expect_err("armed guard must deny");
    ARMED.store(false, Ordering::SeqCst);

    assert_eq!(deny.kind, GatewayDenyKind::Forbidden);
    assert_eq!(deny.retry_after_seconds, 0);

    let seen = SEEN
        .lock()
        .expect("seen lock")
        .take()
        .expect("guard must have observed the request");
    assert_eq!(
        seen,
        (
            "user_2".to_owned(),
            "claude-opus-5".to_owned(),
            Some("route_premium".to_owned()),
            "anthropic".to_owned(),
            true,
        )
    );
}

#[test]
fn deny_reasons_default_to_the_retryable_quota_kind() {
    let deny = GatewayDenyReason::new("balance empty");
    assert_eq!(deny.kind, GatewayDenyKind::Quota);
}
