//! Real credential issuance and enrollment exercise once-only token transport.
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use systemprompt_identifiers::{DeviceCertId, TaskId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_db_pool, seed_user_row,
};
use tower::ServiceExt;
async fn json(router: &Router, request: Request<Body>, status: StatusCode) -> serde_json::Value {
    let uri = request.uri().to_string();
    let response = router.clone().oneshot(request).await.unwrap();
    let actual = response.status();
    let body = to_bytes(response.into_body(), 16384).await.unwrap();
    if actual != status {
        panic!(
            "{uri}: expected {status}, received {actual}: {}",
            String::from_utf8_lossy(&body)
        );
    }
    serde_json::from_slice(&body).unwrap()
}
#[tokio::test]
async fn issued_credential_enrolls_once_retry_omits_token_and_deliberate_rotation_revokes_it() {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await.unwrap();
    let ctx = fixture_app_context(&db, &bootstrap.database_url).unwrap();
    let consumer = UserId::new(uuid::Uuid::new_v4().to_string());
    let cert = DeviceCertId::generate();
    seed_user_row(
        &db,
        ctx.system_admin().id(),
        &format!("{}@api-operation-owner.invalid", ctx.system_admin().id()),
    )
    .await
    .unwrap();
    seed_user_row(
        &db,
        &consumer,
        &format!("{consumer}@api-credential.invalid"),
    )
    .await
    .unwrap();
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO user_device_certs(id,user_id,fingerprint,label) VALUES($1,$2,$3,'HTTP issuance')").bind(cert.as_str()).bind(consumer.as_str()).bind(cert.as_str()).execute(pool.as_ref()).await.unwrap();
    let router = systemprompt_api::routes::evaluation::campaigns::router()
        .with_state(
            systemprompt_api::routes::evaluation::campaigns::OptimizationState::new(
                ctx.as_ref().clone(),
            ),
        )
        .merge(
            systemprompt_api::routes::evaluation::consumer::router()
                .with_state(ctx.as_ref().clone()),
        )
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::evaluation::contract::normalize,
        ));
    let issue = |key: &TaskId| {
        Request::builder()
            .method("POST")
            .uri(format!("/consumer-devices/{cert}/credential"))
            .header("idempotency-key", key.as_str())
            .body(Body::empty())
            .unwrap()
    };
    let enroll = |token: &str| {
        Request::builder()
            .method("POST")
            .uri("/consumer-devices/enrollment")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let key = TaskId::generate();
    let first = json(&router, issue(&key), StatusCode::OK).await;
    let token = first["credential"].as_str().unwrap();
    let identity = json(&router, enroll(token), StatusCode::OK).await;
    assert_eq!(identity["consumer_id"], consumer.as_str());
    assert_eq!(identity["device_id"], cert.as_str());
    let retry = json(&router, issue(&key), StatusCode::OK).await;
    assert!(retry["credential"].is_null());
    assert_eq!(retry["result"], first["result"]);
    assert_eq!(json(&router, enroll(token), StatusCode::OK).await, identity);
    let status = json(
        &router,
        Request::builder()
            .uri(format!("/operations/{key}"))
            .body(Body::empty())
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(status["operation"]["state"], "completed");
    assert!(!status.to_string().contains(token));
    let rotated = json(&router, issue(&TaskId::generate()), StatusCode::OK).await;
    let new_token = rotated["credential"].as_str().unwrap();
    assert_ne!(new_token, token);
    json(&router, enroll(token), StatusCode::UNAUTHORIZED).await;
    assert_eq!(
        json(&router, enroll(new_token), StatusCode::OK).await,
        identity
    );
}
