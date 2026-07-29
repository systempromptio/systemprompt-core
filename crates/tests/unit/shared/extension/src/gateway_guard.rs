//! The gateway request-guard seam: registration, request propagation, and
//! deny-kind semantics.
//!
//! `inventory` registration is binary-wide, so the guard registered here is
//! armed per test via a static and left inert otherwise.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use systemprompt_extension::{
    GatewayDenyKind, GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard,
    register_gateway_guard, run_gateway_guards,
};

static ARMED: AtomicBool = AtomicBool::new(false);
static SEEN: Mutex<Option<(String, String, Option<String>, String, bool)>> = Mutex::new(None);

#[derive(Default)]
struct RecordingGuard;

#[async_trait::async_trait]
impl GatewayRequestGuard for RecordingGuard {
    async fn check(
        &self,
        _pool: &sqlx::PgPool,
        request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        if !ARMED.load(Ordering::SeqCst) {
            return Ok(());
        }
        *SEEN.lock().expect("seen lock") = Some((
            request.user_id.to_owned(),
            request.model.to_owned(),
            request.route_id.map(str::to_owned),
            request.provider.to_owned(),
            request.streaming,
        ));
        Err(GatewayDenyReason::forbidden(
            "your plan does not include this model",
        ))
    }
}

register_gateway_guard!(RecordingGuard);

fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://guard-test-never-connects.invalid/none")
        .expect("lazy pool")
}

#[tokio::test]
async fn an_unarmed_registry_admits_the_request() {
    let pool = lazy_pool();
    let request = GatewayGuardRequest {
        user_id: "user_1",
        model: "claude-opus-5",
        route_id: Some("route_default"),
        provider: "anthropic",
        streaming: false,
    };
    assert!(run_gateway_guards(&pool, &request).await.is_ok());
}

#[tokio::test]
async fn a_guard_sees_the_resolved_request_and_its_forbidden_kind_survives() {
    ARMED.store(true, Ordering::SeqCst);
    let pool = lazy_pool();
    let request = GatewayGuardRequest {
        user_id: "user_2",
        model: "claude-opus-5",
        route_id: Some("route_premium"),
        provider: "anthropic",
        streaming: true,
    };
    let deny = run_gateway_guards(&pool, &request)
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
