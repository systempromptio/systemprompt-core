//! Tests for `POST /v1/bridge/device` — bridge self-enrolment of a device
//! credential: unauthenticated callers are refused, malformed fingerprints
//! are a client error, and a repeat enrolment lands on the same device.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use systemprompt_api::routes::gateway::bridge_device::{SelfEnrollRequest, enroll_self};
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, fixture_app_context, fixture_database_url,
    fixture_db_pool, seed_bridge_credential,
};
use systemprompt_traits::AppContext as _;

struct Harness {
    ctx: Arc<AppContext>,
    extractor: Arc<JwtContextExtractor>,
    authed: AuthedFixture,
}

async fn harness_or_skip() -> Option<Harness> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let ctx = fixture_app_context(&pool, &url).expect("app context");
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().expect("session provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone()),
    ));
    let authed = seed_bridge_credential(&pool, "device@bridge-device.invalid")
        .await
        .expect("bridge credential");
    Some(Harness {
        ctx,
        extractor,
        authed,
    })
}

fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    headers
}

fn fingerprint(seed: char) -> String {
    std::iter::repeat_n(seed, 64).collect()
}

async fn enroll(
    h: &Harness,
    headers: HeaderMap,
    fingerprint: String,
) -> Result<
    Json<systemprompt_api::routes::gateway::bridge_device::SelfEnrollResponse>,
    (StatusCode, String),
> {
    enroll_self(
        Arc::clone(&h.extractor),
        (*h.ctx).clone(),
        headers,
        Json(SelfEnrollRequest {
            fingerprint,
            label: "unit laptop".to_owned(),
        }),
    )
    .await
}

#[tokio::test]
async fn missing_bearer_is_unauthorized() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    let (status, _) = enroll(&h, HeaderMap::new(), fingerprint('a'))
        .await
        .expect_err("no credential must be refused");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn malformed_fingerprint_is_a_client_error() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    let (status, message) = enroll(&h, bearer(h.authed.jwt.as_str()), "deadbeef".to_owned())
        .await
        .expect_err("short fingerprint must be refused");
    assert_eq!(status, StatusCode::BAD_REQUEST, "{message}");
}

#[tokio::test]
async fn self_enrolment_issues_a_device_credential_and_repeats_on_the_same_device() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    // Why: the fingerprint is globally unique, so each run must mint its own.
    let fingerprint =
        systemprompt_models::feedback::ContentDigest::of(h.authed.user_id.as_str().as_bytes())
            .as_str()
            .to_owned();

    let first = enroll(&h, bearer(h.authed.jwt.as_str()), fingerprint.clone())
        .await
        .expect("first enrolment");
    assert_eq!(first.consumer_id, h.authed.user_id);
    assert!(
        first.credential.starts_with("sp_device_"),
        "credential must carry the device prefix"
    );

    let second = enroll(&h, bearer(h.authed.jwt.as_str()), fingerprint)
        .await
        .expect("repeat enrolment");
    assert_eq!(second.device_id, first.device_id);
    assert_eq!(second.consumer_id, first.consumer_id);
    assert_ne!(
        second.credential, first.credential,
        "a repeat enrolment rotates the credential"
    );
}
