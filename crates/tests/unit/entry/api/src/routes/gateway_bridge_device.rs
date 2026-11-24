//! Tests for `POST /v1/bridge/device` — bridge self-enrolment of a device
//! credential: unauthenticated callers are refused, malformed fingerprints
//! are a client error, and a repeat enrolment lands on the same device.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::gateway::SelfEnrollResponse;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, seed_bridge_credential, test_app_context,
    test_database_url, test_db_pool,
};
use tower::ServiceExt;

struct Harness {
    ctx: Arc<AppContext>,
    authed: AuthedFixture,
}

async fn harness() -> Harness {
    let url = test_database_url();
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&url).await;
    let authed = seed_bridge_credential(&pool, "device@bridge-device.invalid")
        .await
        .expect("bridge credential");
    Harness { ctx, authed }
}

fn fingerprint(seed: char) -> String {
    std::iter::repeat_n(seed, 64).collect()
}

async fn enroll(
    h: &Harness,
    bearer: Option<&str>,
    fingerprint: String,
) -> Result<SelfEnrollResponse, (StatusCode, String)> {
    let app = gateway_router(&h.ctx, h.ctx.admitted(), h.ctx.kernel()).expect("gateway router");
    let mut builder = Request::builder()
        .method("POST")
        .uri(ApiPaths::gateway_relative(ApiPaths::BRIDGE_DEVICE))
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let body = serde_json::json!({ "fingerprint": fingerprint, "label": "unit laptop" });
    let response = app
        .oneshot(builder.body(Body::from(body.to_string())).expect("request"))
        .await
        .expect("the device route answers");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    if status.is_success() {
        Ok(serde_json::from_slice(&bytes).expect("an enrolment response"))
    } else {
        Err((status, String::from_utf8_lossy(&bytes).into_owned()))
    }
}

#[tokio::test]
async fn missing_bearer_is_unauthorized() {
    let h = harness().await;
    let (status, body) = enroll(&h, None, fingerprint('a'))
        .await
        .expect_err("no credential must be refused");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("missing_credential"), "{body}");
}

#[tokio::test]
async fn malformed_fingerprint_is_a_client_error() {
    let h = harness().await;
    let (status, body) = enroll(&h, Some(h.authed.jwt.as_str()), "deadbeef".to_owned())
        .await
        .expect_err("short fingerprint must be refused");
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn a_rejected_credential_is_unauthorized_without_the_decoder_cause() {
    let h = harness().await;
    let (status, body) = enroll(&h, Some("not-a-jwt"), fingerprint('b'))
        .await
        .expect_err("a forged credential must be refused");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("invalid_credential"), "{body}");
    assert!(body.contains("Bridge credential rejected"), "{body}");
}

#[tokio::test]
async fn self_enrolment_issues_a_device_credential_and_repeats_on_the_same_device() {
    let h = harness().await;
    // Why: the fingerprint is globally unique, so each run must mint its own.
    let fingerprint =
        systemprompt_models::feedback::ContentDigest::of(h.authed.user_id.as_str().as_bytes())
            .as_str()
            .to_owned();

    let first = enroll(&h, Some(h.authed.jwt.as_str()), fingerprint.clone())
        .await
        .expect("first enrolment");
    assert_eq!(first.consumer_id, h.authed.user_id);
    assert!(
        first.credential.starts_with("sp_device_"),
        "credential must carry the device prefix"
    );

    let second = enroll(&h, Some(h.authed.jwt.as_str()), fingerprint)
        .await
        .expect("repeat enrolment");
    assert_eq!(second.device_id, first.device_id);
    assert_eq!(second.consumer_id, first.consumer_id);
    assert_ne!(
        second.credential, first.credential,
        "a repeat enrolment rotates the credential"
    );
}
