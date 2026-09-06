//! `POST /v1/gateway/auth/mtls` — device-certificate exchange.
//!
//! This endpoint turns a client-certificate fingerprint into a bridge access
//! token, so the refusals below are the whole authentication boundary: a
//! caller who sends no fingerprint, one whose fingerprint is not a SHA-256
//! digest at all, and one whose well-formed fingerprint was never enrolled.
//! All three must be refused before any token is minted, and they must stay
//! distinguishable — a missing or misshapen field is the caller's mistake, an
//! unknown certificate is a rejected credential.

use axum::Json;
use axum::response::IntoResponse;
use systemprompt_api::routes::gateway::auth::{MtlsRequestBody, mtls};
use systemprompt_api::services::middleware::client_addr::ClientIp;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};

async fn context() -> std::sync::Arc<systemprompt_runtime::AppContext> {
    let boot = ensure_test_bootstrap();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    fixture_app_context(&pool, &boot.database_url).expect("fixture context")
}

#[tokio::test]
async fn a_request_with_no_fingerprint_is_a_bad_request_not_an_unauthorized() {
    let ctx = context().await;

    let error = mtls(
        (*ctx).clone(),
        ClientIp(None),
        axum::http::HeaderMap::new(),
        Json(MtlsRequestBody {
            device_cert_fingerprint: String::new(),
        }),
    )
    .await
    .err()
    .expect("an empty fingerprint cannot authenticate anything");

    assert_eq!(
        error.into_response().status(),
        axum::http::StatusCode::BAD_REQUEST,
        "a malformed request is the caller's to fix; answering 401 would send them hunting for a \
         credential problem they do not have"
    );
}

// Why: whitespace is trimmed before the emptiness check, so a fingerprint of
// spaces must be refused the same way. Without the trim it would fall through
// to a certificate lookup for a blank fingerprint.
#[tokio::test]
async fn a_fingerprint_of_only_whitespace_is_refused_as_missing() {
    let ctx = context().await;

    let error = mtls(
        (*ctx).clone(),
        ClientIp(None),
        axum::http::HeaderMap::new(),
        Json(MtlsRequestBody {
            device_cert_fingerprint: "   ".to_owned(),
        }),
    )
    .await
    .err()
    .expect("a blank fingerprint is not a fingerprint");

    assert_eq!(
        error.into_response().status(),
        axum::http::StatusCode::BAD_REQUEST
    );
}

// Why: the fingerprint is normalised and shape-checked before any lookup, so
// a value that is not 64 hex characters never reaches the certificate table.
// That is a different refusal from an unrecognised certificate, and the two
// must not be collapsed — one tells the caller their client is misconfigured,
// the other that their device is not enrolled.
#[tokio::test]
async fn a_malformed_fingerprint_is_rejected_on_shape_before_any_lookup() {
    let ctx = context().await;

    let error = mtls(
        (*ctx).clone(),
        ClientIp(None),
        axum::http::HeaderMap::new(),
        Json(MtlsRequestBody {
            device_cert_fingerprint: "sha256:not-a-hex-digest".to_owned(),
        }),
    )
    .await
    .err()
    .expect("a fingerprint that is not a SHA-256 digest cannot identify a certificate");

    assert_eq!(
        error.into_response().status(),
        axum::http::StatusCode::BAD_REQUEST,
        "a wrongly-shaped fingerprint is a client misconfiguration, not a rejected credential"
    );
}

#[tokio::test]
async fn a_well_formed_fingerprint_that_was_never_enrolled_is_refused_as_unauthorized() {
    let ctx = context().await;
    let never_enrolled = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    assert_eq!(
        never_enrolled.len(),
        64,
        "the fingerprint must be well formed or the shape check refuses it first"
    );

    let error = mtls(
        (*ctx).clone(),
        ClientIp(None),
        axum::http::HeaderMap::new(),
        Json(MtlsRequestBody {
            device_cert_fingerprint: never_enrolled,
        }),
    )
    .await
    .err()
    .expect("an unenrolled device certificate must not yield a bridge token");

    assert_eq!(
        error.into_response().status(),
        axum::http::StatusCode::UNAUTHORIZED,
        "an unknown or revoked certificate is a rejected credential, not a malformed request"
    );
}
