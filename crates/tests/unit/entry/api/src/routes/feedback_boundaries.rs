//! The consumer router enforces device identity and cookie origin.
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use tower::ServiceExt;
async fn routers() -> (Router, String) {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await.unwrap();
    let ctx = fixture_app_context(&db, &bootstrap.database_url).unwrap();
    let origin = url::Url::parse(&ctx.config().api_external_url)
        .unwrap()
        .origin()
        .ascii_serialization();
    let consumer = systemprompt_api::routes::managed::consumer::router()
        .layer(axum::middleware::from_fn_with_state(
            ctx.as_ref().clone(),
            systemprompt_api::routes::managed::origin::protect,
        ))
        .with_state(ctx.as_ref().clone())
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));
    (consumer, origin)
}
#[tokio::test]
async fn submitted_identity_user_secret_and_unenrolled_credentials_do_not_authenticate() {
    let (consumer, origin) = routers().await;
    for token in ["user-bridge-secret", "eyJ.user.jwt", "sp_device_forged"] {
        let response = consumer
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/consumer-devices/enrollment")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"device_id":"forged","consumer_id":"publisher"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let problem: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(problem["type"], "about:blank");
        assert!(problem["detail"].is_string());
    }
    for supplied in [
        None,
        Some("https://untrusted.invalid"),
        Some(origin.as_str()),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/consumer-devices/enrollment")
            .header("cookie", "access_token=unused");
        if let Some(origin) = supplied {
            request = request.header("origin", origin);
        }
        let response = consumer
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if supplied == Some(origin.as_str()) {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::FORBIDDEN
            }
        );
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
    }
}
