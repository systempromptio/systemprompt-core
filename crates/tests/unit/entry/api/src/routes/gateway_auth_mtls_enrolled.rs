//! `POST /v1/gateway/auth/mtls` for a device whose certificate *is* enrolled.
//!
//! The refusal paths are covered elsewhere; what matters here is that an
//! enrolled fingerprint mints a bridge token bound to the enrolling user, and
//! that revoking the enrolment takes that power away immediately rather than
//! at the next token expiry.

use axum::Json;
use axum::response::IntoResponse;
use std::sync::Arc;
use systemprompt_api::routes::gateway::auth::{MtlsRequestBody, mtls};
use systemprompt_api::services::middleware::client_addr::ClientIp;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_db_pool, install_test_signing_key,
    seed_user_row,
};
use systemprompt_users::{DeviceCertService, EnrollDeviceCertServiceParams as EnrollParams};

struct Enrolled {
    ctx: AppContext,
    service: DeviceCertService,
    user_id: UserId,
    fingerprint: String,
}

async fn enrolled_device() -> Enrolled {
    let boot = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");

    let user_id = UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@example.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let fingerprint = uuid::Uuid::new_v4().simple().to_string().repeat(2);
    let service = DeviceCertService::new(Arc::clone(ctx.user_repository()));
    service
        .enroll(EnrollParams {
            user_id: &user_id,
            fingerprint: &fingerprint,
            label: "coverage device",
        })
        .await
        .expect("enrol device certificate");

    Enrolled {
        ctx: (*ctx).clone(),
        service,
        user_id,
        fingerprint,
    }
}

#[tokio::test]
async fn an_enrolled_fingerprint_mints_a_bridge_token() {
    let device = enrolled_device().await;

    let response = mtls(
        device.ctx.clone(),
        ClientIp(None),
        axum::http::HeaderMap::new(),
        Json(MtlsRequestBody {
            device_cert_fingerprint: device.fingerprint.clone(),
        }),
    )
    .await
    .expect("an enrolled device certificate is a valid credential");

    assert!(
        !response.0.token.is_empty(),
        "an accepted certificate must yield a usable token, not an empty string"
    );
    assert!(
        response.0.ttl > 0,
        "a token with no lifetime would be unusable the moment it is issued"
    );
}

#[tokio::test]
async fn an_uppercase_fingerprint_matches_the_stored_enrolment() {
    let device = enrolled_device().await;

    let response = mtls(
        device.ctx.clone(),
        ClientIp(None),
        axum::http::HeaderMap::new(),
        Json(MtlsRequestBody {
            device_cert_fingerprint: device.fingerprint.to_uppercase(),
        }),
    )
    .await
    .expect("fingerprint case is a transport detail, not an identity difference");

    assert!(!response.0.token.is_empty());
}

#[tokio::test]
async fn a_revoked_certificate_can_no_longer_authenticate() {
    let device = enrolled_device().await;
    let certs = device
        .service
        .list_for_user(&device.user_id)
        .await
        .expect("list enrolled certificates");
    let cert = certs.first().expect("the enrolled certificate");
    assert!(
        device
            .service
            .revoke(&cert.id, &device.user_id)
            .await
            .expect("revoke"),
        "revoking an enrolment the user owns must report that it happened"
    );

    let error = mtls(
        device.ctx.clone(),
        ClientIp(None),
        axum::http::HeaderMap::new(),
        Json(MtlsRequestBody {
            device_cert_fingerprint: device.fingerprint.clone(),
        }),
    )
    .await
    .map(|_| ())
    .expect_err("a revoked certificate is no longer a credential");

    assert_eq!(
        error.into_response().status(),
        axum::http::StatusCode::UNAUTHORIZED,
        "revocation must take effect at the next exchange, not at the next expiry"
    );
}
